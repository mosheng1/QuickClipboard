#[cfg(windows)]
mod windows_text_drag {
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;
    use tauri::WebviewWindow;
    use windows::core::{implement, Error, HRESULT, PCWSTR};
    use windows_core::{BOOL, Ref};
    use windows::Win32::Foundation::{DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, DV_E_FORMATETC, E_NOTIMPL, OLE_E_ADVISENOTSUPPORTED, S_OK};
    use windows::Win32::System::Com::{IAdviseSink, IDataObject, IDataObject_Impl, IEnumFORMATETC, IEnumSTATDATA, DVASPECT_CONTENT, FORMATETC, STGMEDIUM, STGMEDIUM_0, TYMED_HGLOBAL};
    use windows::Win32::System::DataExchange::RegisterClipboardFormatW;
    use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
    use windows::Win32::System::Ole::{DoDragDrop, IDropSource, IDropSource_Impl, OleInitialize, OleUninitialize, CF_UNICODETEXT, DROPEFFECT, DROPEFFECT_COPY};
    use windows::Win32::System::SystemServices::{MK_LBUTTON, MODIFIERKEYS_FLAGS};

    static HTML_FORMAT: std::sync::OnceLock<u16> = std::sync::OnceLock::new();

    fn html_format_id() -> u16 {
        *HTML_FORMAT.get_or_init(|| unsafe {
            let name: Vec<u16> = std::ffi::OsStr::new("HTML Format").encode_wide().chain(once(0)).collect();
            RegisterClipboardFormatW(PCWSTR(name.as_ptr())) as u16
        })
    }

    fn medium(bytes: &[u8]) -> windows::core::Result<STGMEDIUM> {
        unsafe {
            let handle = GlobalAlloc(GMEM_MOVEABLE, bytes.len())?;
            let ptr = GlobalLock(handle) as *mut u8;
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len());
            let _ = GlobalUnlock(handle);
            Ok(STGMEDIUM { tymed: TYMED_HGLOBAL.0 as u32, u: STGMEDIUM_0 { hGlobal: handle }, pUnkForRelease: std::mem::ManuallyDrop::new(None) })
        }
    }

    fn cf_html(html: &str) -> Vec<u8> {
        let prefix = "<html><body>\r\n<!--StartFragment-->";
        let suffix = "<!--EndFragment-->\r\n</body></html>";
        let header = "Version:0.9\r\nStartHTML:0000000000\r\nEndHTML:0000000000\r\nStartFragment:0000000000\r\nEndFragment:0000000000\r\n";
        let start_html = header.len();
        let start_fragment = start_html + prefix.len();
        let end_fragment = start_fragment + html.len();
        let end_html = end_fragment + suffix.len();
        let header = format!("Version:0.9\r\nStartHTML:{start_html:010}\r\nEndHTML:{end_html:010}\r\nStartFragment:{start_fragment:010}\r\nEndFragment:{end_fragment:010}\r\n");
        format!("{header}{prefix}{html}{suffix}").into_bytes()
    }

    #[implement(IDataObject)]
    struct TextDataObject { plain: Vec<u8>, html: Option<Vec<u8>> }

    impl TextDataObject {
        fn format(&self, ptr: *const FORMATETC) -> Option<u16> {
            let f = unsafe { ptr.as_ref()? };
            if f.tymed as i32 != TYMED_HGLOBAL.0 || f.dwAspect != DVASPECT_CONTENT.0 { return None; }
            if f.cfFormat == CF_UNICODETEXT.0 { return Some(CF_UNICODETEXT.0); }
            if self.html.is_some() && f.cfFormat == html_format_id() { return Some(f.cfFormat); }
            None
        }
    }

    #[allow(non_snake_case)]
    impl IDataObject_Impl for TextDataObject_Impl {
        fn GetData(&self, format: *const FORMATETC) -> windows::core::Result<STGMEDIUM> {
            match self.format(format) {
                Some(cf) if cf == CF_UNICODETEXT.0 => medium(&self.plain),
                Some(_) => medium(self.html.as_ref().unwrap()),
                None => Err(Error::new(DV_E_FORMATETC, "不支持的数据格式")),
            }
        }
        fn GetDataHere(&self, _: *const FORMATETC, _: *mut STGMEDIUM) -> windows::core::Result<()> { Err(Error::new(E_NOTIMPL, "不支持直接写入")) }
        fn QueryGetData(&self, format: *const FORMATETC) -> HRESULT { if self.format(format).is_some() { S_OK } else { DV_E_FORMATETC } }
        fn GetCanonicalFormatEtc(&self, _: *const FORMATETC, out: *mut FORMATETC) -> HRESULT { unsafe { (*out).ptd = std::ptr::null_mut(); } E_NOTIMPL }
        fn SetData(&self, _: *const FORMATETC, _: *const STGMEDIUM, _: BOOL) -> windows::core::Result<()> { Err(Error::new(E_NOTIMPL, "不支持写入数据")) }
        fn EnumFormatEtc(&self, _: u32) -> windows::core::Result<IEnumFORMATETC> { Err(Error::new(E_NOTIMPL, "不支持枚举格式")) }
        fn DAdvise(&self, _: *const FORMATETC, _: u32, _: Ref<IAdviseSink>) -> windows::core::Result<u32> { Err(Error::new(OLE_E_ADVISENOTSUPPORTED, "不支持数据通知")) }
        fn DUnadvise(&self, _: u32) -> windows::core::Result<()> { Err(Error::new(OLE_E_ADVISENOTSUPPORTED, "不支持取消数据通知")) }
        fn EnumDAdvise(&self) -> windows::core::Result<IEnumSTATDATA> { Err(Error::new(OLE_E_ADVISENOTSUPPORTED, "不支持枚举数据通知")) }
    }

    #[implement(IDropSource)]
    struct TextDropSource;

    #[allow(non_snake_case)]
    impl IDropSource_Impl for TextDropSource_Impl {
        fn QueryContinueDrag(&self, escape: BOOL, keys: MODIFIERKEYS_FLAGS) -> HRESULT {
            if escape.as_bool() { DRAGDROP_S_CANCEL } else if (keys & MK_LBUTTON) == MODIFIERKEYS_FLAGS(0) { DRAGDROP_S_DROP } else { S_OK }
        }
        fn GiveFeedback(&self, _: DROPEFFECT) -> HRESULT { windows::Win32::Foundation::DRAGDROP_S_USEDEFAULTCURSORS }
    }

    pub fn start(window: WebviewWindow, plain: String, html: Option<String>) -> Result<(), String> {
        if plain.is_empty() { return Err("文本内容为空".to_string()); }
        let (sender, receiver) = std::sync::mpsc::channel();

        window.run_on_main_thread(move || {
            let result = unsafe {
                match OleInitialize(Some(std::ptr::null_mut())) {
                    Ok(()) => {
                    let plain: Vec<u16> = std::ffi::OsStr::new(&plain).encode_wide().chain(once(0)).collect();
                    let bytes = std::slice::from_raw_parts(plain.as_ptr() as *const u8, plain.len() * 2).to_vec();
                    let object: IDataObject = TextDataObject { plain: bytes, html: html.map(|value| cf_html(&value)) }.into();
                    let source: IDropSource = TextDropSource.into();
                    let mut effect = DROPEFFECT::default();
                    let drag_result = DoDragDrop(&object, &source, DROPEFFECT_COPY, &mut effect).ok();
                    OleUninitialize();
                    drag_result
                    }
                    Err(error) => Err(error),
                }
            };
            let _ = sender.send(result.map_err(|error| format!("文本拖拽失败: {error}")));
        }).map_err(|error| format!("启动文本拖拽失败: {error}"))?;

        receiver.recv().map_err(|_| "文本拖拽已被中断".to_string())?
    }
}

#[tauri::command]
pub fn start_text_drag(window: tauri::WebviewWindow, plain: String, html: Option<String>) -> Result<(), String> {
    #[cfg(windows)]
    { return windows_text_drag::start(window, plain, html); }
    #[cfg(not(windows))]
    { let _ = (window, plain, html); Err("当前平台暂不支持文本原生拖拽".to_string()) }
}
