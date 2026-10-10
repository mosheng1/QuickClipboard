// OCR 识别命令

// OCR识别结果结构
#[cfg(windows)]
#[derive(Debug, serde::Serialize)]
pub struct OcrWord {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[cfg(windows)]
#[derive(Debug, serde::Serialize)]
pub struct OcrLine {
    pub text: String,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub words: Vec<OcrWord>,
    pub word_gaps: Vec<f32>,
}

#[cfg(windows)]
#[derive(Debug, serde::Serialize)]
pub struct OcrResult {
    pub text: String,
    pub lines: Vec<OcrLine>,
}

// OCR识别图片字节数组
#[cfg(windows)]
#[tauri::command]
pub async fn recognize_image_ocr(image_data: Vec<u8>) -> Result<OcrResult, String> {
    tokio::task::spawn_blocking(move || {
        use qcocr::recognize_from_bytes;

        let result =
            recognize_from_bytes(&image_data, None).map_err(|e| format!("OCR识别失败: {}", e))?;

        convert_ocr_result(result)
    })
    .await
    .map_err(|e| format!("任务执行失败: {}", e))?
}

// OCR识别图片文件
#[cfg(windows)]
#[tauri::command]
pub async fn recognize_file_ocr(
    file_path: String,
    language: Option<String>,
) -> Result<OcrResult, String> {
    tokio::task::spawn_blocking(move || {
        use qcocr::recognize_from_file;

        let lang = language.as_deref();
        let result =
            recognize_from_file(&file_path, lang).map_err(|e| format!("OCR识别失败: {}", e))?;

        convert_ocr_result(result)
    })
    .await
    .map_err(|e| format!("任务执行失败: {}", e))?
}

// 转换OCR结果为返回格式
#[cfg(windows)]
fn convert_ocr_result(result: qcocr::OcrRecognitionResult) -> Result<OcrResult, String> {
    let lines = result
        .lines
        .iter()
        .map(|line| {
            let words = line
                .words
                .iter()
                .map(|word| OcrWord {
                    text: word.text.clone(),
                    x: word.bounds.x,
                    y: word.bounds.y,
                    width: word.bounds.width,
                    height: word.bounds.height,
                })
                .collect();

            let word_gaps = line.compute_word_gaps();

            OcrLine {
                text: line.text.clone(),
                x: line.bounds.x,
                y: line.bounds.y,
                width: line.bounds.width,
                height: line.bounds.height,
                words,
                word_gaps,
            }
        })
        .collect();

    Ok(OcrResult {
        text: result.text,
        lines,
    })
}
