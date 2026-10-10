use once_cell::sync::Lazy;
use rodio::cpal::traits::{DeviceTrait, HostTrait};
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use std::fs::File;
use std::io::{BufReader, Cursor};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

static BUILTIN_COPY_SOUND: &[u8] = include_bytes!("../../../../sounds/copy.mp3");
static BUILTIN_PASTE_SOUND: &[u8] = include_bytes!("../../../../sounds/paste.mp3");
static BUILTIN_SCROLL_SOUND: &[u8] = include_bytes!("../../../../sounds/roll.mp3");
const AUDIO_WARMUP_DURATION: Duration = Duration::from_millis(200);
const AUDIO_IDLE_RELEASE_DELAY: Duration = Duration::from_secs(1);
const AUDIO_POLL_INTERVAL: Duration = Duration::from_millis(50);

// 记录最后一次粘贴音效播放的时间戳
static LAST_PASTE_SOUND_TIME_MS: AtomicU64 = AtomicU64::new(0);

fn current_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_millis() as u64
}

pub fn mark_paste_operation() {
    LAST_PASTE_SOUND_TIME_MS.store(current_time_ms(), Ordering::Relaxed);
}

enum SoundCommand {
    PlayFile(PathBuf, f32),
    PlayBytes(&'static [u8], f32),
    PlayBeep(f32, u64, f32),
}

static SOUND_SENDER: Lazy<Sender<SoundCommand>> = Lazy::new(|| {
    let (tx, rx) = mpsc::channel::<SoundCommand>();

    if let Err(error) = thread::Builder::new()
        .name("audio-player".into())
        .spawn(move || audio_thread_loop(rx))
    {
        eprintln!("创建音效播放线程失败: {}", error);
    }

    tx
});

fn get_default_device_name() -> Option<String> {
    rodio::cpal::default_host()
        .default_output_device()
        .and_then(|device| device.name().ok())
}

struct AudioContext {
    _stream: OutputStream,
    handle: OutputStreamHandle,
    device_name: Option<String>,
    sinks: Vec<Sink>,
}

impl AudioContext {
    fn try_new() -> Option<Self> {
        let (stream, handle) = OutputStream::try_default().ok()?;
        let device_name = get_default_device_name();
        Some(Self {
            _stream: stream,
            handle,
            device_name,
            sinks: Vec::new(),
        })
    }

    fn device_changed(&self) -> bool {
        get_default_device_name() != self.device_name
    }

    fn play(&mut self, cmd: &SoundCommand) -> Result<(), String> {
        let sink = match cmd {
            SoundCommand::PlayFile(path, volume) => play_file(&self.handle, path, *volume),
            SoundCommand::PlayBytes(bytes, volume) => play_bytes(&self.handle, bytes, *volume),
            SoundCommand::PlayBeep(freq, dur, vol) => play_beep(&self.handle, *freq, *dur, *vol),
        }?;

        self.sinks.push(sink);
        Ok(())
    }

    fn remove_finished_sinks(&mut self) {
        self.sinks.retain(|sink| !sink.empty());
    }

    fn is_idle(&self) -> bool {
        self.sinks.is_empty()
    }
}

fn create_audio_context() -> Option<AudioContext> {
    let context = AudioContext::try_new();
    if context.is_some() {
        thread::sleep(AUDIO_WARMUP_DURATION);
    }
    context
}

fn audio_thread_loop(rx: mpsc::Receiver<SoundCommand>) {
    let mut ctx: Option<AudioContext> = None;
    let mut idle_since: Option<Instant> = None;

    loop {
        let timeout = if ctx.is_some() {
            AUDIO_POLL_INTERVAL
        } else {
            Duration::from_secs(1)
        };

        match rx.recv_timeout(timeout) {
            Ok(cmd) => {
                if ctx
                    .as_ref()
                    .map_or(true, |context| context.device_changed())
                {
                    ctx = create_audio_context();
                }

                let result = ctx
                    .as_mut()
                    .map_or(Err("无音频设备".to_string()), |context| {
                        context.play(&cmd)
                    });

                if result.is_err() {
                    ctx = create_audio_context();
                    if let Some(context) = ctx.as_mut() {
                        let _ = context.play(&cmd);
                    }
                }

                idle_since = None;
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }

        let should_release = match ctx.as_mut() {
            Some(context) => {
                context.remove_finished_sinks();
                if context.is_idle() {
                    match idle_since {
                        Some(since) => since.elapsed() >= AUDIO_IDLE_RELEASE_DELAY,
                        None => {
                            idle_since = Some(Instant::now());
                            false
                        }
                    }
                } else {
                    idle_since = None;
                    false
                }
            }
            None => false,
        };

        if should_release {
            ctx = None;
            idle_since = None;
        }
    }
}

fn play_file(handle: &OutputStreamHandle, path: &PathBuf, volume: f32) -> Result<Sink, String> {
    let sink = Sink::try_new(handle).map_err(|e| e.to_string())?;
    let file = File::open(path).map_err(|e| format!("打开文件失败: {}", e))?;
    let source = Decoder::new(BufReader::new(file)).map_err(|e| format!("解码失败: {}", e))?;

    sink.set_volume(volume);
    sink.append(source);
    Ok(sink)
}

fn play_bytes(
    handle: &OutputStreamHandle,
    bytes: &'static [u8],
    volume: f32,
) -> Result<Sink, String> {
    let sink = Sink::try_new(handle).map_err(|e| e.to_string())?;
    let source = Decoder::new(Cursor::new(bytes)).map_err(|e| format!("解码失败: {}", e))?;

    sink.set_volume(volume);
    sink.append(source);
    Ok(sink)
}

fn play_beep(
    handle: &OutputStreamHandle,
    frequency: f32,
    duration_ms: u64,
    volume: f32,
) -> Result<Sink, String> {
    let sink = Sink::try_new(handle).map_err(|e| e.to_string())?;

    let sample_rate = 44100u32;
    let duration_samples = ((sample_rate as f64 * duration_ms as f64) / 1000.0) as usize;
    let two_pi_freq = 2.0 * std::f32::consts::PI * frequency;
    let sample_rate_f = sample_rate as f32;

    let samples: Vec<f32> = (0..duration_samples)
        .map(|i| (two_pi_freq * i as f32 / sample_rate_f).sin())
        .collect();

    let source = rodio::buffer::SamplesBuffer::new(1, sample_rate, samples);
    sink.set_volume(volume);
    sink.append(source);
    Ok(sink)
}

#[inline]
fn send_command(cmd: SoundCommand) {
    let _ = SOUND_SENDER.send(cmd);
}

pub struct SoundPlayer;

impl SoundPlayer {
    #[inline]
    pub fn play(path: impl AsRef<std::path::Path>, volume: f32) {
        send_command(SoundCommand::PlayFile(path.as_ref().to_path_buf(), volume));
    }

    #[inline]
    pub fn play_bytes(bytes: &'static [u8], volume: f32) {
        send_command(SoundCommand::PlayBytes(bytes, volume));
    }

    #[inline]
    pub fn play_beep(frequency: f32, duration_ms: u64, volume: f32) {
        send_command(SoundCommand::PlayBeep(frequency, duration_ms, volume));
    }
}

pub struct AppSounds;

impl AppSounds {
    // 复制音效 - 成功时播放
    pub fn play_copy_on_success() {
        let settings = crate::get_settings();
        if settings.copy_sound_timing != "success" {
            return;
        }
        Self::do_play_copy(&settings);
    }

    // 复制音效 - 立即播放
    pub fn play_copy_immediate() {
        let settings = crate::get_settings();
        if settings.copy_sound_timing != "immediate" {
            return;
        }

        let last_paste_time = LAST_PASTE_SOUND_TIME_MS.load(Ordering::Relaxed);
        if last_paste_time > 0 {
            let current_time = current_time_ms();
            if current_time.saturating_sub(last_paste_time) < 300 {
                return;
            }
        }

        Self::do_play_copy(&settings);
    }

    fn do_play_copy(settings: &crate::services::AppSettings) {
        if !settings.sound_enabled {
            return;
        }

        let volume = (settings.sound_volume / 100.0) as f32;

        if !settings.copy_sound_path.is_empty() {
            let path = Self::resolve_path(&settings.copy_sound_path);
            if path.exists() {
                SoundPlayer::play(path, volume);
                return;
            }
        }

        SoundPlayer::play_bytes(BUILTIN_COPY_SOUND, volume);
    }

    // 粘贴音效 - 成功时播放
    pub fn play_paste_on_success() {
        let settings = crate::get_settings();
        if settings.paste_sound_timing != "success" {
            return;
        }

        LAST_PASTE_SOUND_TIME_MS.store(current_time_ms(), Ordering::Relaxed);

        Self::do_play_paste(&settings);
    }

    // 粘贴音效 - 立即播放
    pub fn play_paste_immediate() {
        let settings = crate::get_settings();
        if settings.paste_sound_timing != "immediate" {
            return;
        }

        LAST_PASTE_SOUND_TIME_MS.store(current_time_ms(), Ordering::Relaxed);

        Self::do_play_paste(&settings);
    }

    fn do_play_paste(settings: &crate::services::AppSettings) {
        if !settings.sound_enabled {
            return;
        }

        let volume = (settings.sound_volume / 100.0) as f32;

        if !settings.paste_sound_path.is_empty() {
            let path = Self::resolve_path(&settings.paste_sound_path);
            if path.exists() {
                SoundPlayer::play(path, volume);
                return;
            }
        }

        SoundPlayer::play_bytes(BUILTIN_PASTE_SOUND, volume);
    }

    pub fn play_copy() {
        let settings = crate::get_settings();
        Self::do_play_copy(&settings);
    }

    pub fn play_paste() {
        let settings = crate::get_settings();
        Self::do_play_paste(&settings);
    }

    pub fn play_scroll() {
        let settings = crate::get_settings();
        if !settings.sound_enabled || !settings.quickpaste_scroll_sound {
            return;
        }

        let volume = (settings.sound_volume / 100.0) as f32;

        if !settings.quickpaste_scroll_sound_path.is_empty() {
            let path = Self::resolve_path(&settings.quickpaste_scroll_sound_path);
            if path.exists() {
                SoundPlayer::play(path, volume);
                return;
            }
        }

        SoundPlayer::play_bytes(BUILTIN_SCROLL_SOUND, volume);
    }

    fn resolve_path(path: &str) -> PathBuf {
        let p = std::path::Path::new(path);

        if p.is_absolute() {
            return p.to_path_buf();
        }

        crate::get_data_directory()
            .map(|dir| dir.join(path))
            .unwrap_or_else(|_| p.to_path_buf())
    }
}
