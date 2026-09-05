//! Windows 打印：Windows.Data.Pdf 优先，PDFium 兜底，最后经 Win32 打印 API 输出

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;

use pdfium_render::prelude::*;
use windows::Data::Pdf::{PdfDocument as WinRtPdfDocument, PdfPageRenderOptions};
use windows::Foundation::Size;
use windows::Graphics::Imaging::{
    BitmapAlphaMode, BitmapDecoder, BitmapPixelFormat, BitmapTransform, ColorManagementMode,
    ExifOrientationMode,
};
use windows::Storage::Streams::{DataWriter, InMemoryRandomAccessStream};
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateDCW, DEVMODEW, DIB_RGB_COLORS, DM_IN_BUFFER,
    DM_ORIENTATION, DM_OUT_BUFFER, DM_PAPERSIZE, DeleteDC, GetDeviceCaps, HDC, HORZRES,
    PHYSICALHEIGHT, PHYSICALWIDTH, SRCCOPY, STRETCH_HALFTONE, SetStretchBltMode, StretchDIBits,
    VERTRES,
};
use windows::Win32::Graphics::Printing::{
    ClosePrinter, DocumentPropertiesW, OpenPrinterW, PRINTER_HANDLE,
};
use windows::Win32::Storage::Xps::{AbortDoc, DOCINFOW, EndDoc, EndPage, StartDocW, StartPage};
use windows::core::{Interface, PCWSTR};
use windows::Win32::Foundation::{GetLastError, ERROR_CANCELLED, ERROR_PRINT_CANCELLED};

use super::printer::PRINT_CANCELLED_ERR;

const PRINT_DPI: f32 = 300.0;
const MAX_PIXELS: u64 = 40_000_000;
const DMPAPER_A4: i16 = 9;
const DMPAPER_A5: i16 = 11;
const DMORIENT_PORTRAIT: i16 = 1;
const DMORIENT_LANDSCAPE: i16 = 2;

/// 上一次 Win32 调用错误是「用户取消打印」时返回标记错误，否则返回默认错误文案
///
/// 覆盖两类取消：
/// - ERROR_PRINT_CANCELLED（63）：假脱机程序里取消打印作业
/// - ERROR_CANCELLED（1223）：XPS Document Writer 等弹出「保存输出」对话框后被取消，
///   StartDocW 以此错误码失败
fn cancelled_or_default(default: &str) -> String {
    unsafe {
        let code = GetLastError();
        if code == ERROR_PRINT_CANCELLED || code == ERROR_CANCELLED {
            return PRINT_CANCELLED_ERR.to_string();
        }
    }
    default.to_string()
}

struct PageBitmap {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

/// 打印 PDF：优先使用系统组件，系统组件失败时使用 PDFium 兜底
pub fn print_pdf(
    file_path: &str,
    printer: Option<&str>,
    paper: Option<&str>,
    orientation: Option<&str>,
) -> Result<(), String> {
    let printer_name = match printer.map(str::trim).filter(|name| !name.is_empty()) {
        Some(name) => name.to_string(),
        None => super::printer::default_printer_name().unwrap_or_default(),
    };
    if printer_name.is_empty() {
        return Err("未检测到可用打印机，请联系工作人员".into());
    }

    let bytes = std::fs::read(file_path).map_err(|e| format!("读取 PDF 文件失败: {e}"))?;

    if let Err(primary_error) = print_with_windows_data(&bytes, &printer_name, paper, orientation) {
        // 用户主动取消：不走 PDFium 兜底，直接交由 UI 静默处理
        if primary_error == PRINT_CANCELLED_ERR {
            return Err(primary_error);
        }
        super::log::warn(
            "printer-win",
            &format!("Windows.Data.Pdf 打印失败，准备使用 PDFium 兜底: {primary_error}"),
        );
        return print_with_pdfium(file_path, &printer_name, paper, orientation).map_err(|e| {
            format!("Windows.Data.Pdf 打印失败: {primary_error}；PDFium 兜底失败: {e}")
        });
    }

    Ok(())
}

/// 使用 WinRT Windows.Data.Pdf 光栅化并打印
fn print_with_windows_data(
    bytes: &[u8],
    printer_name: &str,
    paper: Option<&str>,
    orientation: Option<&str>,
) -> Result<(), String> {
    let document = load_pdf_document(bytes)?;
    let page_count = document
        .PageCount()
        .map_err(|e| format!("读取 PDF 页数失败: {e}"))?;
    if page_count == 0 {
        return Err("PDF 没有可打印的页面".into());
    }

    let hdc = begin_print_job(printer_name, paper, orientation)?;
    let mut result = Ok(());

    for index in 0..page_count {
        match render_page_with_winrt(&document, index).and_then(|page| draw_page(hdc, &page)) {
            Ok(()) => {}
            Err(e) => {
                result = Err(e);
                break;
            }
        }
    }

    unsafe {
        if result.is_ok() {
            EndDoc(hdc);
        } else {
            let _ = AbortDoc(hdc);
        }
        let _ = DeleteDC(hdc);
    }

    result
}

/// 载入 PDF 文档（WinRT Windows.Data.Pdf）
fn load_pdf_document(bytes: &[u8]) -> Result<WinRtPdfDocument, String> {
    let stream = InMemoryRandomAccessStream::new().map_err(|e| format!("创建内存流失败: {e}"))?;
    let writer =
        DataWriter::CreateDataWriter(&stream).map_err(|e| format!("创建数据写入器失败: {e}"))?;
    writer
        .WriteBytes(bytes)
        .map_err(|e| format!("写入 PDF 数据失败: {e}"))?;

    let store = writer
        .StoreAsync()
        .map_err(|e| format!("提交 PDF 数据失败: {e}"))?;
    let store_async = store
        .cast::<windows_future::IAsyncOperation<u32>>()
        .map_err(|e| format!("等待 PDF 数据提交失败: {e}"))?;
    store_async
        .get()
        .map_err(|e| format!("等待 PDF 数据提交失败: {e}"))?;
    writer
        .FlushAsync()
        .map_err(|e| format!("刷新 PDF 数据失败: {e}"))?
        .get()
        .map_err(|e| format!("刷新 PDF 数据失败: {e}"))?;

    WinRtPdfDocument::LoadFromStreamAsync(&stream)
        .map_err(|e| format!("加载 PDF 文档失败: {e}"))?
        .get()
        .map_err(|e| format!("加载 PDF 文档失败: {e}"))
}

/// 使用随应用分发的 PDFium 动态库光栅化并打印
fn print_with_pdfium(
    file_path: &str,
    printer_name: &str,
    paper: Option<&str>,
    orientation: Option<&str>,
) -> Result<(), String> {
    let pdfium = create_pdfium()?;
    let document = pdfium
        .load_pdf_from_file(file_path, None)
        .map_err(|e| format!("加载 PDF 文档失败: {e}"))?;
    let page_count = document.pages().len();
    if page_count == 0 {
        return Err("PDF 没有可打印的页面".into());
    }

    let hdc = begin_print_job(&printer_name, paper, orientation)?;
    let mut result = Ok(());

    for index in 0..page_count {
        match render_page_with_pdfium(&document, index).and_then(|page| draw_page(hdc, &page)) {
            Ok(()) => {}
            Err(e) => {
                result = Err(e);
                break;
            }
        }
    }

    unsafe {
        if result.is_ok() {
            EndDoc(hdc);
        } else {
            let _ = AbortDoc(hdc);
        }
        let _ = DeleteDC(hdc);
    }

    result
}

/// 初始化随应用分发的 PDFium 动态库
fn create_pdfium() -> Result<Pdfium, String> {
    let library_path = find_pdfium_library().ok_or_else(|| {
        "未找到 pdfium.dll；请确认安装包完整，或设置 PIS_PDFIUM_PATH 指向该文件".to_string()
    })?;

    match Pdfium::bind_to_library(&library_path) {
        Ok(bindings) => Ok(Pdfium::new(bindings)),
        Err(PdfiumError::PdfiumLibraryBindingsAlreadyInitialized) => Ok(Pdfium::default()),
        Err(error) => Err(format!(
            "加载 PDFium 动态库失败（{}）: {error}",
            library_path.display()
        )),
    }
}

/// 优先使用显式配置，其次查找应用目录内随安装包分发的动态库
fn find_pdfium_library() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("PIS_PDFIUM_PATH") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }

    let exe_dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let library_name = Pdfium::pdfium_platform_library_name();
    [
        exe_dir.join(&library_name),
        exe_dir.join("bin").join(&library_name),
        exe_dir.join("pdfium").join(library_name),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

/// 渲染指定页为 GDI 可直接使用的 32 位 BGRA 位图
fn render_page_with_pdfium(document: &PdfDocument<'_>, index: i32) -> Result<PageBitmap, String> {
    let page = document
        .pages()
        .get(index)
        .map_err(|e| format!("读取 PDF 第 {} 页失败: {e}", index + 1))?;
    let page_width = page.width().value.max(0.0);
    let page_height = page.height().value.max(0.0);
    let mut width = (page_width * PRINT_DPI / 72.0).round() as u32;
    let mut height = (page_height * PRINT_DPI / 72.0).round() as u32;
    while width > 1 && height > 1 && (width as u64 * height as u64) > MAX_PIXELS {
        width /= 2;
        height /= 2;
    }

    let bitmap = page
        .render_with_config(
            &PdfRenderConfig::new()
                .set_fixed_size(width.max(1) as i32, height.max(1) as i32)
                .set_format(PdfBitmapFormat::BGRA)
                .set_reverse_byte_order(false)
                .use_print_quality(true)
                .set_clear_color(PdfColor::WHITE),
        )
        .map_err(|e| format!("渲染 PDF 第 {} 页失败: {e}", index + 1))?;

    let raw = bitmap.as_raw_bytes();
    let stride = raw.len() / bitmap.height().max(1) as usize;
    let row_bytes = bitmap.width().max(1) as usize * 4;
    let mut pixels = Vec::with_capacity(row_bytes * bitmap.height().max(1) as usize);
    for row in 0..bitmap.height().max(1) as usize {
        let start = row * stride;
        pixels.extend_from_slice(&raw[start..start + row_bytes]);
    }

    Ok(PageBitmap {
        width: bitmap.width().max(1) as u32,
        height: bitmap.height().max(1) as u32,
        pixels,
    })
}

/// 使用 Windows.Data.Pdf 渲染指定页为 BGRA 位图
fn render_page_with_winrt(document: &WinRtPdfDocument, index: u32) -> Result<PageBitmap, String> {
    let page = document
        .GetPage(index)
        .map_err(|e| format!("读取 PDF 第 {} 页失败: {e}", index + 1))?;
    let Size { Width, Height } = page
        .Size()
        .map_err(|e| format!("读取 PDF 页面尺寸失败: {e}"))?;

    let mut width = (Width * PRINT_DPI / 72.0).round() as u32;
    let mut height = (Height * PRINT_DPI / 72.0).round() as u32;
    while width > 1 && height > 1 && (width as u64 * height as u64) > MAX_PIXELS {
        width /= 2;
        height /= 2;
    }

    let options = PdfPageRenderOptions::new().map_err(|e| format!("创建渲染参数失败: {e}"))?;
    options
        .SetDestinationWidth(width.max(1))
        .map_err(|e| format!("设置渲染宽度失败: {e}"))?;
    options
        .SetDestinationHeight(height.max(1))
        .map_err(|e| format!("设置渲染高度失败: {e}"))?;

    let output = InMemoryRandomAccessStream::new().map_err(|e| format!("创建渲染流失败: {e}"))?;
    page.RenderWithOptionsToStreamAsync(&output, &options)
        .map_err(|e| format!("渲染 PDF 第 {} 页失败: {e}", index + 1))?
        .get()
        .map_err(|e| format!("渲染 PDF 第 {} 页失败: {e}", index + 1))?;

    let decoder = BitmapDecoder::CreateAsync(&output)
        .map_err(|e| format!("解析 PDF 第 {} 页位图失败: {e}", index + 1))?
        .get()
        .map_err(|e| format!("解析 PDF 第 {} 页位图失败: {e}", index + 1))?;

    // 新版 Windows 会校验 transform 指针，传 None（空指针）会报 E_POINTER
    // （0x80004003），必须传入恒等变换对象
    let transform = BitmapTransform::new()
        .map_err(|e| format!("创建位图变换失败: {e}"))?;

    let provider = decoder
        .GetPixelDataTransformedAsync(
            BitmapPixelFormat::Bgra8,
            BitmapAlphaMode::Premultiplied,
            Some(&transform),
            ExifOrientationMode::IgnoreExifOrientation,
            ColorManagementMode::DoNotColorManage,
        )
        .map_err(|e| format!("读取 PDF 第 {} 页像素失败: {e}", index + 1))?
        .get()
        .map_err(|e| format!("读取 PDF 第 {} 页像素失败: {e}", index + 1))?;

    let pixels = provider
        .DetachPixelData()
        .map_err(|e| format!("读取 PDF 第 {} 页像素失败: {e}", index + 1))?;

    Ok(PageBitmap {
        width: decoder
            .PixelWidth()
            .map_err(|e| format!("读取位图宽度失败: {e}"))?,
        height: decoder
            .PixelHeight()
            .map_err(|e| format!("读取位图高度失败: {e}"))?,
        pixels: pixels.to_vec(),
    })
}

/// 开始打印作业：构造应用了纸张与方向的设备上下文
fn begin_print_job(
    printer_name: &str,
    paper: Option<&str>,
    orientation: Option<&str>,
) -> Result<HDC, String> {
    let name_wide = to_wide(printer_name);
    let driver_wide = to_wide("WINSPOOL");
    let empty_wide = [0u16];

    unsafe {
        let mut handle = PRINTER_HANDLE::default();
        OpenPrinterW(PCWSTR(name_wide.as_ptr()), &mut handle, None)
            .map_err(|e| format!("打开打印机失败: {e}"))?;

        let size = DocumentPropertiesW(None, handle, PCWSTR(name_wide.as_ptr()), None, None, 0);
        if size < 1 {
            let _ = ClosePrinter(handle);
            return Err("获取打印机配置大小失败".into());
        }

        let mut buffer = vec![0u8; size as usize];
        let devmode = buffer.as_mut_ptr() as *mut DEVMODEW;
        if DocumentPropertiesW(
            None,
            handle,
            PCWSTR(name_wide.as_ptr()),
            Some(devmode),
            None,
            DM_OUT_BUFFER.0,
        ) < 0
        {
            let _ = ClosePrinter(handle);
            return Err("读取打印机默认配置失败".into());
        }

        {
            let dm = &mut *devmode;
            dm.dmFields |= DM_PAPERSIZE | DM_ORIENTATION;
            let fields = &mut dm.Anonymous1.Anonymous1;
            fields.dmPaperSize = match paper.map(str::trim).unwrap_or("A4") {
                "A5" => DMPAPER_A5,
                _ => DMPAPER_A4,
            };
            fields.dmOrientation = if orientation.map(str::trim) == Some("landscape") {
                DMORIENT_LANDSCAPE
            } else {
                DMORIENT_PORTRAIT
            };
        }

        DocumentPropertiesW(
            None,
            handle,
            PCWSTR(name_wide.as_ptr()),
            Some(devmode),
            Some(devmode),
            DM_IN_BUFFER.0 | DM_OUT_BUFFER.0,
        );
        let _ = ClosePrinter(handle);

        let hdc = CreateDCW(
            PCWSTR(driver_wide.as_ptr()),
            PCWSTR(name_wide.as_ptr()),
            PCWSTR(empty_wide.as_ptr()),
            Some(devmode),
        );
        if hdc.is_invalid() {
            return Err("创建打印机上下文失败".into());
        }

        let doc_name = to_wide("病理报告");
        // lpszOutput / lpszDatatype 必须为 NULL（空字符串会被后台程序当作
        // 无效的输出文件名，导致 StartDoc 直接失败）
        let docinfo = DOCINFOW {
            cbSize: std::mem::size_of::<DOCINFOW>() as i32,
            lpszDocName: PCWSTR(doc_name.as_ptr()),
            lpszOutput: PCWSTR::null(),
            lpszDatatype: PCWSTR::null(),
            fwType: 0,
        };
        if StartDocW(hdc, &docinfo) <= 0 {
            let _ = DeleteDC(hdc);
            return Err(cancelled_or_default("启动打印作业失败"));
        }

        SetStretchBltMode(hdc, STRETCH_HALFTONE);
        Ok(hdc)
    }
}

/// 输出一页：等比缩放到可打印区域并居中
fn draw_page(hdc: HDC, page: &PageBitmap) -> Result<(), String> {
    unsafe {
        if StartPage(hdc) <= 0 {
            return Err("开始打印页面失败".into());
        }

        let horz = GetDeviceCaps(Some(hdc), HORZRES);
        let vert = GetDeviceCaps(Some(hdc), VERTRES);
        let area_width = if horz > 0 {
            horz
        } else {
            GetDeviceCaps(Some(hdc), PHYSICALWIDTH)
        };
        let area_height = if vert > 0 {
            vert
        } else {
            GetDeviceCaps(Some(hdc), PHYSICALHEIGHT)
        };

        let scale = (area_width as f64 / page.width.max(1) as f64)
            .min(area_height as f64 / page.height.max(1) as f64);
        let draw_width = (page.width as f64 * scale).round() as i32;
        let draw_height = (page.height as f64 * scale).round() as i32;
        let dest_x = (area_width - draw_width) / 2;
        let dest_y = (area_height - draw_height) / 2;

        let header = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: page.width as i32,
            // 负高度表示自上而下的位图，避免纵向翻转
            biHeight: -(page.height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            biSizeImage: 0,
            biXPelsPerMeter: 0,
            biYPelsPerMeter: 0,
            biClrUsed: 0,
            biClrImportant: 0,
        };
        let bitmap_info = BITMAPINFO {
            bmiHeader: header,
            bmiColors: [std::mem::zeroed()],
        };

        let written = StretchDIBits(
            hdc,
            dest_x,
            dest_y,
            draw_width,
            draw_height,
            0,
            0,
            page.width as i32,
            page.height as i32,
            Some(page.pixels.as_ptr() as *const std::ffi::c_void),
            &bitmap_info,
            DIB_RGB_COLORS,
            SRCCOPY,
        );
        EndPage(hdc);

        if written == 0 {
            // 用户在系统打印进度对话框点了「取消」不算错误，交给 UI 静默处理
            return Err(cancelled_or_default("输出打印页面失败"));
        }
    }
    Ok(())
}

/// 转为以 \0 结尾的 UTF-16 字符串
fn to_wide(value: &str) -> Vec<u16> {
    OsStr::new(value)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}
