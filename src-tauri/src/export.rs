use std::{
    io::Read,
    path::{Path, PathBuf},
};

use anyhow::Context;
use lopdf::{
    content::{Content, Operation},
    dictionary, Document, Object, Stream,
};
use tauri::AppHandle;
use tauri_specta::Event;

use crate::{events::ExportPdfEvent, extensions::PathIsImg, types::Comic};

enum Archive {
    Pdf,
}
impl Archive {
    pub fn extension(&self) -> &str {
        match self {
            Archive::Pdf => "pdf",
        }
    }
}

struct PdfEventGuard {
    uuid: String,
    app: AppHandle,
    success: bool,
}

impl Drop for PdfEventGuard {
    fn drop(&mut self) {
        let uuid = self.uuid.clone();

        let _ = if self.success {
            ExportPdfEvent::End { uuid }.emit(&self.app)
        } else {
            ExportPdfEvent::Error { uuid }.emit(&self.app)
        };
    }
}

pub fn pdf(app: &AppHandle, comic: &Comic) -> anyhow::Result<()> {
    let comic_title = &comic.title;
    let event_uuid = uuid::Uuid::new_v4().to_string();

    let _ = ExportPdfEvent::Start {
        uuid: event_uuid.clone(),
        title: comic_title.clone(),
    }
    .emit(app);

    // Event guard to ensure that the error event is sent if the function panics
    let mut pdf_event_guard = PdfEventGuard {
        uuid: event_uuid.clone(),
        app: app.clone(),
        success: false,
    };

    let download_dir = comic.get_download_dir(app);
    let export_dir = comic.get_export_dir(app);
    // Ensure export directory exists
    std::fs::create_dir_all(&export_dir).context(format!(
        "Failed to create directory `{}`",
        export_dir.display()
    ))?;
    // Create PDF
    let extension = Archive::Pdf.extension();
    let dir_name = &comic.dir_name;
    let pdf_path = export_dir.join(format!("{dir_name}.{extension}"));
    create_pdf(&download_dir, &pdf_path).context("Failed to create PDF")?;
    // Set success to true to ensure that the end event is sent
    pdf_event_guard.success = true;

    Ok(())
}

/// Create a PDF with images from `comic_download_dir` and save it to `pdf_path`
#[allow(clippy::similar_names)]
#[allow(clippy::cast_possible_truncation)]
fn create_pdf(comic_download_dir: &Path, pdf_path: &Path) -> anyhow::Result<()> {
    let mut image_paths: Vec<PathBuf> = std::fs::read_dir(comic_download_dir)
        .context(format!(
            "Failed to read directory `{}`",
            comic_download_dir.display()
        ))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_img()) // Filter out metadata.json files
        .collect();
    image_paths.sort_by(|a, b| a.file_name().cmp(&b.file_name()));

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let mut page_ids = vec![];

    for image_path in image_paths {
        if !image_path.is_file() {
            continue;
        }

        let buffer = read_image_to_buffer(&image_path).context(format!(
            "Failed to read `{}` into buffer",
            image_path.display()
        ))?;
        let (width, height) = image::image_dimensions(&image_path).context(format!(
            "Failed to get dimensions of `{}`",
            image_path.display()
        ))?;
        let image_stream = lopdf::xobject::image_from(buffer).context(format!(
            "Failed to create image stream for `{}`",
            image_path.display()
        ))?;
        // Add image stream to doc
        let img_id = doc.add_object(image_stream);
        // Image name for the Do operation to display the image on the page
        let img_name = format!("X{}", img_id.0);
        // Used to set image position and size on the page
        let cm_operation = Operation::new(
            "cm",
            vec![
                width.into(),
                0.into(),
                0.into(),
                height.into(),
                0.into(),
                0.into(),
            ],
        );
        // Used to display the image
        let do_operation = Operation::new("Do", vec![Object::Name(img_name.as_bytes().to_vec())]);
        // Create a page, set the image position and size, and then display the image
        // Since we're creating a PDF from scratch, there's no need to use q and Q operations to save and restore graphics state
        let content = Content {
            operations: vec![cm_operation, do_operation],
        };
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode()?));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "MediaBox" => vec![0.into(), 0.into(), width.into(), height.into()],
        });
        // Add the image as XObject to the document
        // The Do operation can only reference XObject (that's why we defined the Do operation with img_name as parameter, not img_id)
        doc.add_xobject(page_id, img_name.as_bytes(), img_id)?;
        // Record the ID of the newly created page
        page_ids.push(page_id);
    }
    // Add "Pages" to the doc
    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Count" => page_ids.len() as u32,
        "Kids" => page_ids.into_iter().map(Object::Reference).collect::<Vec<_>>(),
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));
    // Create a new "Catalog" object, add the "Pages" object to the "Catalog" object, then add the "Catalog" object to doc
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    doc.compress();

    doc.save(pdf_path)
        .context(format!("Failed to save `{}`", pdf_path.display()))?;
    Ok(())
}

/// Read image data from `image_path` into a buffer
fn read_image_to_buffer(image_path: &Path) -> anyhow::Result<Vec<u8>> {
    let file = std::fs::File::open(image_path)
        .context(format!("Failed to open `{}`", image_path.display()))?;
    let mut reader = std::io::BufReader::new(file);
    let mut buffer = vec![];
    reader
        .read_to_end(&mut buffer)
        .context(format!("Failed to read `{}`", image_path.display()))?;
    Ok(buffer)
}
