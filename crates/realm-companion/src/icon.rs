use bevy::prelude::*;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
#[derive(Resource)]
pub(crate) struct AppIcon {
    pub path: PathBuf,
    pub menu_wow: Vec<u8>,
    pub classes: HashMap<u8, Vec<u8>>,
}
fn read_image(chain: &benilla_formats::Chain, name: &str) -> Result<image::RgbaImage, String> {
    let bytes = chain.read(name).map_err(|e| e.to_string())?;
    let decoded = benilla_blp::decode(&bytes).map_err(|e| e.to_string())?;
    let m = &decoded.mips[0];
    image::RgbaImage::from_raw(m.width, m.height, m.rgba.clone())
        .ok_or("invalid BLP dimensions".into())
}
fn menu_size(image: image::RgbaImage) -> Vec<u8> {
    let small = image::DynamicImage::ImageRgba8(image)
        .resize(18, 18, image::imageops::FilterType::Lanczos3)
        .to_rgba8();
    let mut square = image::RgbaImage::new(18, 18);
    image::imageops::overlay(
        &mut square,
        &small,
        i64::from((18 - small.width()) / 2),
        i64::from((18 - small.height()) / 2),
    );
    square.into_raw()
}
/// Exact UV boundaries from stock GlueXML/CharacterCreate.lua CLASS_ICON_TCOORDS.
fn class_rect(class: u8) -> Option<(u32, u32, u32, u32)> {
    match class {
        1 => Some((0, 0, 64, 64)),
        8 => Some((64, 0, 63, 64)),
        4 => Some((127, 0, 63, 64)),
        11 => Some((190, 0, 63, 64)),
        3 => Some((0, 64, 64, 64)),
        7 => Some((64, 64, 63, 64)),
        5 => Some((127, 64, 63, 64)),
        9 => Some((190, 64, 63, 64)),
        2 => Some((0, 128, 64, 64)),
        _ => None,
    }
}
fn app_image(image:image::RgbaImage)->image::RgbaImage {
    let resized=image::DynamicImage::ImageRgba8(image).resize(896,896,image::imageops::FilterType::Lanczos3).to_rgba8();
    let mut square=image::RgbaImage::new(1024,1024);
    image::imageops::overlay(&mut square,&resized,i64::from((1024-resized.width())/2),i64::from((1024-resized.height())/2));square
}
pub fn export(data:&Path,path:&Path)->Result<(),String>{
    let chain=benilla_formats::Chain::open(data).map_err(|e|e.to_string())?;
    let wow=read_image(&chain,"Interface\\Glues\\Common\\Glues-WoW-Logo.blp")?;
    if let Some(parent)=path.parent(){std::fs::create_dir_all(parent).map_err(|e|e.to_string())?;}
    app_image(wow).save(path).map_err(|e|e.to_string())
}
pub fn load(data: &Path, config: &Path) -> Result<AppIcon, String> {
    let chain = benilla_formats::Chain::open(data).map_err(|e| e.to_string())?;
    let wow = read_image(&chain, "Interface\\Glues\\Common\\Glues-WoW-Logo.blp")?;
    let path = config
        .parent()
        .ok_or("settings path")?
        .join("generated/wow-app-icon.png");
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    app_image(wow.clone()).save(&path).map_err(|e| e.to_string())?;
    let atlas = read_image(
        &chain,
        "Interface\\Glues\\CharacterCreate\\UI-CharacterCreate-Classes.blp",
    )?;
    let mut classes = HashMap::new();
    for class in [1, 2, 3, 4, 5, 7, 8, 9, 11] {
        let (x, y, w, h) = class_rect(class).unwrap();
        classes.insert(
            class,
            menu_size(image::imageops::crop_imm(&atlas, x, y, w, h).to_image()),
        );
    }
    Ok(AppIcon {
        path,
        menu_wow: menu_size(wow),
        classes,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn app_logo_is_square_and_preserves_transparent_margin(){
        let source=image::RgbaImage::from_pixel(200,100,image::Rgba([255,200,0,255]));let image=app_image(source);
        assert_eq!(image.dimensions(),(1024,1024));assert_eq!(image.get_pixel(0,0).0[3],0);assert_eq!(image.get_pixel(512,512).0[3],255);
    }
    #[test]
    fn menu_classes_follow_stock_glue_atlas_bounds() {
        assert_eq!(class_rect(8), Some((64, 0, 63, 64)));
        assert_eq!(class_rect(9), Some((190, 64, 63, 64)));
        for c in [1, 2, 3, 4, 5, 7, 8, 9, 11] {
            let (x, y, w, h) = class_rect(c).unwrap();
            assert!(x + w <= 256 && y + h <= 256);
        }
        assert_eq!(class_rect(0), None);
        assert_eq!(class_rect(6), None);
        let bytes = menu_size(image::RgbaImage::new(256, 128));
        assert_eq!(bytes.len(), 18 * 18 * 4);
    }
}
