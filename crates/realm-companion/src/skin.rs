//! WoW artwork is loaded read-only from the user's patch chain, never bundled.
//! Frame border layout follows benilla-ui's stock backdrop (`0x77e8d0`/`0x77f0c0`);
//! button caps follow the install's UIPanelButtonTemplate2 (UIPanelTemplates.xml).
use bevy_egui::egui::{self, Color32, FontFamily, FontId, Pos2, Rect, TextureHandle, Vec2};
use std::path::Path;

pub const GOLD: Color32 = Color32::from_rgb(255, 209, 0);
#[derive(Clone)]
pub struct WowSkin {
    background: TextureHandle,
    border: TextureHandle,
    up: TextureHandle,
    down: TextureHandle,
    disabled: TextureHandle,
    highlight: TextureHandle,
    role_highlight: TextureHandle,
    selector_logo: Option<TextureHandle>,
    selector_horde: Option<TextureHandle>,
    selector_alliance: Option<TextureHandle>,
    check_up: TextureHandle,
    check_down: TextureHandle,
    check_mark: TextureHandle,
    slider_knob: TextureHandle,
    scroll_knob: TextureHandle,
    fonts: egui::FontDefinitions,
}
impl WowSkin {
    /// Call once an egui context exists. No archive writes, network or game login.
    pub fn load(data_dir: &Path, ctx: &egui::Context) -> Result<Self, String> {
        let chain = benilla_formats::Chain::open(data_dir).map_err(|e| e.to_string())?;
        let texture = |name: &str| -> Result<TextureHandle, String> {
            let bytes = chain
                .read(&format!("{name}.blp"))
                .map_err(|e| e.to_string())?;
            let decoded = benilla_blp::decode(&bytes).map_err(|e| format!("{name}: {e:?}"))?;
            let mip = decoded.mips.first().ok_or("BLP has no base level")?;
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [mip.width as usize, mip.height as usize],
                &mip.rgba,
            );
            Ok(ctx.load_texture(name, image, egui::TextureOptions::LINEAR))
        };
        let mut fonts = egui::FontDefinitions::default();
        let mut installed = Vec::new();
        // Chinese install's own font first, then the stock Latin face and local CJK fallback.
        for (name, bytes) in [
            ("wow-zh", read_font(&chain, data_dir, "Fonts\\FZLBJW.TTF")),
            (
                "wow-latin",
                read_font(&chain, data_dir, "Fonts\\FRIZQT__.TTF"),
            ),
        ] {
            if let Some(bytes) = bytes {
                fonts
                    .font_data
                    .insert(name.into(), egui::FontData::from_owned(bytes).into());
                installed.push(name.to_owned());
            }
        }
        if let Some(bytes) = system_cjk_font() {
            fonts
                .font_data
                .insert("local-cjk".into(), egui::FontData::from_owned(bytes).into());
            installed.push("local-cjk".into());
        }
        if installed.is_empty() {
            return Err("未找到游戏字体，请选择完整的 WoW Data 目录".into());
        }
        for family in [FontFamily::Proportional, FontFamily::Monospace] {
            let existing = fonts.families.entry(family).or_default();
            installed
                .iter()
                .rev()
                .for_each(|name| existing.insert(0, name.clone()));
        }
        Ok(Self {
            selector_logo: texture("Interface\\Glues\\Common\\Glues-WoW-Logo").ok(),
            selector_horde: texture("Interface\\PvPRankBadges\\PvPRankHorde").ok(),
            selector_alliance: texture("Interface\\PvPRankBadges\\PvPRankAlliance").ok(),
            background: texture("Interface\\DialogFrame\\UI-DialogBox-Background")?,
            border: texture("Interface\\DialogFrame\\UI-DialogBox-Border")?,
            up: texture("Interface\\Buttons\\UI-Panel-Button-Up")?,
            down: texture("Interface\\Buttons\\UI-Panel-Button-Down")?,
            disabled: texture("Interface\\Buttons\\UI-Panel-Button-Disabled")?,
            highlight: texture("Interface\\Buttons\\UI-Panel-Button-Highlight")?,
            role_highlight: texture(
                "Interface\\Glues\\CharacterSelect\\Glues-CharacterSelect-Highlight",
            )?,
            check_up: texture("Interface\\Buttons\\UI-CheckBox-Up")?,
            check_down: texture("Interface\\Buttons\\UI-CheckBox-Down")?,
            check_mark: texture("Interface\\Buttons\\UI-CheckBox-Check")?,
            slider_knob: texture("Interface\\Buttons\\UI-SliderBar-Button-Horizontal")?,
            scroll_knob: texture("Interface\\Buttons\\UI-ScrollBar-Knob")?,
            fonts,
        })
    }
    /// Apply once per new context, not every frame (font atlas rebuilds otherwise).
    pub fn apply(&self, ctx: &egui::Context) {
        ctx.set_fonts(self.fonts.clone());
        ctx.style_mut(|s| {
            s.spacing.item_spacing = Vec2::new(10.0, 9.0);
            s.spacing.button_padding = Vec2::new(16.0, 6.0);
            s.spacing.interact_size.y = 26.0;
            s.spacing.scroll=egui::style::ScrollStyle::solid();
            s.spacing.scroll.bar_width=18.;
            s.spacing.scroll.handle_min_length=36.;
            s.visuals.widgets.inactive.bg_fill=Color32::from_rgb(105,79,36);
            s.visuals.widgets.hovered.bg_fill=Color32::from_rgb(155,119,48);
            s.visuals.widgets.active.bg_fill=Color32::from_rgb(190,148,55);
            s.visuals.dark_mode = true;
            s.visuals.override_text_color = Some(Color32::from_rgb(238, 231, 210));
            s.visuals.window_fill = Color32::TRANSPARENT;
            s.visuals.panel_fill = Color32::TRANSPARENT;
            s.visuals.extreme_bg_color = Color32::from_rgb(10, 10, 10);
            s.visuals.selection.bg_fill = Color32::from_rgb(91, 34, 21);
            s.visuals.selection.stroke.color = GOLD;
            s.text_styles
                .insert(egui::TextStyle::Heading, FontId::proportional(21.0));
            s.text_styles
                .insert(egui::TextStyle::Body, FontId::proportional(16.0));
            s.text_styles
                .insert(egui::TextStyle::Button, FontId::proportional(16.0));
        });
    }
    /// Paint before the frame's controls. Rect is the outer edge, with 32-point stock border.
    pub fn paint_frame(&self, ui: &egui::Ui, rect: Rect) {
        if rect.width() < 1.0 || rect.height() < 1.0 {
            return;
        }
        let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
        let background = rect.shrink(10.0);
        tiles(
            &painter,
            self.background.id(),
            background,
            32.0,
            |r, part| {
                (
                    r,
                    [
                        Pos2::new(0.0, 0.0),
                        Pos2::new(part.x, 0.0),
                        Pos2::new(part.x, part.y),
                        Pos2::new(0.0, part.y),
                    ],
                )
            },
        );
        let edge = 32.0f32.min(rect.width() / 2.0).min(rect.height() / 2.0);
        let corners = [
            (4, Rect::from_min_size(rect.left_top(), Vec2::splat(edge))),
            (
                5,
                Rect::from_min_size(rect.right_top() - Vec2::new(edge, 0.0), Vec2::splat(edge)),
            ),
            (
                6,
                Rect::from_min_size(rect.left_bottom() - Vec2::new(0.0, edge), Vec2::splat(edge)),
            ),
            (
                7,
                Rect::from_min_size(rect.right_bottom() - Vec2::splat(edge), Vec2::splat(edge)),
            ),
        ];
        for (slice, r) in corners {
            quad(
                &painter,
                self.border.id(),
                r,
                atlas_uv(slice, 1.0, false),
                Color32::WHITE,
            );
        }
        let sides = [
            (
                0,
                Rect::from_min_max(
                    rect.min + Vec2::new(0., edge),
                    Pos2::new(rect.left() + edge, rect.bottom() - edge),
                ),
                false,
            ),
            (
                1,
                Rect::from_min_max(
                    Pos2::new(rect.right() - edge, rect.top() + edge),
                    rect.max - Vec2::new(0., edge),
                ),
                false,
            ),
            (
                2,
                Rect::from_min_max(
                    rect.min + Vec2::new(edge, 0.),
                    Pos2::new(rect.right() - edge, rect.top() + edge),
                ),
                true,
            ),
            (
                3,
                Rect::from_min_max(
                    Pos2::new(rect.left() + edge, rect.bottom() - edge),
                    rect.max - Vec2::new(edge, 0.),
                ),
                true,
            ),
        ];
        for (slice, r, horizontal) in sides {
            let length = if horizontal { r.width() } else { r.height() };
            let mut at = 0.0;
            while at < length {
                let n = edge.min(length - at);
                let segment = if horizontal {
                    Rect::from_min_size(r.min + Vec2::new(at, 0.), Vec2::new(n, edge))
                } else {
                    Rect::from_min_size(r.min + Vec2::new(0., at), Vec2::new(edge, n))
                };
                quad(
                    &painter,
                    self.border.id(),
                    segment,
                    atlas_uv(slice, n / edge, horizontal),
                    Color32::WHITE,
                );
                at += n;
            }
        }
    }
    /// Standard egui keyboard/mouse activation and enabled semantics, with stock 3-piece artwork.
    pub fn button(&self, ui: &mut egui::Ui, label: &str) -> egui::Response {
        let text_width = ui
            .painter()
            .layout_no_wrap(label.to_owned(), FontId::proportional(16.0), GOLD)
            .size()
            .x;
        let response = ui.add(
            egui::Button::new(label)
                .frame(false)
                .min_size(Vec2::new((text_width + 32.0).max(80.0), 28.0)),
        );
        if ui.is_rect_visible(response.rect) {
            let enabled = ui.is_enabled();
            let texture = if !enabled {
                &self.disabled
            } else if response.is_pointer_button_down_on() {
                &self.down
            } else {
                &self.up
            };
            button_three(ui.painter(), texture.id(), response.rect, Color32::WHITE);
            if enabled && (response.hovered() || response.has_focus()) {
                button_three(
                    ui.painter(),
                    self.highlight.id(),
                    response.rect,
                    Color32::from_white_alpha(115),
                );
            }
            let offset = if response.is_pointer_button_down_on() {
                Vec2::new(1.0, 1.0)
            } else {
                Vec2::ZERO
            };
            ui.painter().text(
                response.rect.center() + offset + Vec2::new(1., 1.),
                egui::Align2::CENTER_CENTER,
                label,
                FontId::proportional(16.0),
                Color32::BLACK,
            );
            ui.painter().text(
                response.rect.center() + offset,
                egui::Align2::CENTER_CENTER,
                label,
                FontId::proportional(16.0),
                if enabled { GOLD } else { Color32::GRAY },
            );
        }
        response
    }
    pub fn paint_selector(&self,ui:&egui::Ui,rect:Rect,race:Option<u8>){
        let canvas=rect.shrink(12.);let right=Rect::from_min_max(Pos2::new(canvas.right()-392.,canvas.top()+8.),canvas.right_bottom()-Vec2::splat(8.));
        let left=Rect::from_min_max(canvas.left_top()+Vec2::new(24.,24.),Pos2::new(right.left()-20.,canvas.bottom()-24.));
        if let Some(logo)=&self.selector_logo{
            let target=Rect::from_center_size(Pos2::new(left.center().x,left.top()+left.height()*0.3),Vec2::new(left.width(),left.width()*0.5));
            ui.painter().image(logo.id(),target,Rect::from_min_max(Pos2::ZERO,Pos2::new(1.,1.)),Color32::WHITE);
        }
        let faction=if matches!(race,Some(2|5|6|8)){self.selector_horde.as_ref()}else if race.is_some(){self.selector_alliance.as_ref()}else{None};
        if let Some(image)=faction{
            let target=Rect::from_center_size(Pos2::new(left.center().x,left.top()+left.height()*0.72),Vec2::splat(130.));
            ui.painter().image(image.id(),target,Rect::from_min_max(Pos2::ZERO,Pos2::new(1.,1.)),Color32::from_white_alpha(160));
        }
        ui.painter().rect_filled(right,8.,Color32::from_black_alpha(160));
        ui.painter().rect_stroke(right,8.,egui::Stroke::new(1.,Color32::from_rgb(110,91,56)),egui::StrokeKind::Inside);
    }
    pub fn character_row(
        &self,
        ui: &mut egui::Ui,
        name: &str,
        subtitle: &str,
        selected: bool,
    ) -> egui::Response {
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(ui.available_width().min(340.), 48.0), egui::Sense::click());
        ui.painter().image(
            self.background.id(),
            rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
            Color32::WHITE,
        );
        if selected {
            ui.painter().image(
                self.role_highlight.id(),
                rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.)),
                Color32::WHITE,
            );
        }
        ui.painter().text(
            rect.left_top() + Vec2::new(20., 10.),
            egui::Align2::LEFT_TOP,
            name,
            FontId::proportional(22.),
            if selected {
                GOLD
            } else {
                Color32::from_rgb(235, 225, 205)
            },
        );
        if name != subtitle {
            ui.painter().text(
                rect.left_top() + Vec2::new(20., 31.),
                egui::Align2::LEFT_TOP,
                subtitle,
                FontId::proportional(11.),
                Color32::GRAY,
            );
        }
        response
    }
    pub fn scale_slider(&self,ui:&mut egui::Ui,value:&mut f32)->egui::Response {
        let (rect,mut response)=ui.allocate_exact_size(Vec2::new(180.,24.),egui::Sense::click_and_drag());
        if (response.dragged()||response.clicked())&&response.interact_pointer_pos().is_some(){
            let point=response.interact_pointer_pos().unwrap();*value=slider_value(point.x,rect.left()+8.,rect.right()-8.);response.mark_changed();
        }
        let track=Rect::from_center_size(rect.center(),Vec2::new(rect.width()-12.,6.));
        ui.painter().rect_filled(track,2.,Color32::from_rgb(25,20,12));
        ui.painter().rect_stroke(track,2.,egui::Stroke::new(1.,Color32::from_rgb(132,107,63)),egui::StrokeKind::Outside);
        let x=egui::lerp((rect.left()+8.)..=(rect.right()-8.),((*value-25.)/375.).clamp(0.,1.));
        ui.painter().image(self.slider_knob.id(),Rect::from_center_size(Pos2::new(x,rect.center().y),Vec2::new(24.,28.)),Rect::from_min_max(Pos2::ZERO,Pos2::new(1.,1.)),Color32::WHITE);response
    }
    pub fn scroll_art(&self,ui:&egui::Ui,inner:Rect,content:Vec2,offset:Vec2){
        if content.y<=inner.height(){return;}
        let track=Rect::from_min_size(Pos2::new(inner.right()+4.,inner.top()),Vec2::new(18.,inner.height()));
        ui.painter().rect_filled(track,1.,Color32::from_rgb(15,12,8));
        ui.painter().rect_stroke(track,1.,egui::Stroke::new(1.,Color32::from_rgb(110,96,74)),egui::StrokeKind::Inside);
        let height=(inner.height()*inner.height()/content.y).max(36.).min(inner.height());
        let y=track.top()+(track.height()-height)*(offset.y/(content.y-inner.height())).clamp(0.,1.);
        ui.painter().image(self.scroll_knob.id(),Rect::from_min_size(Pos2::new(track.left(),y),Vec2::new(18.,height)),Rect::from_min_max(Pos2::ZERO,Pos2::new(1.,1.)),Color32::WHITE);
    }
    pub fn checkbox(&self, ui: &mut egui::Ui, checked: &mut bool, label: &str) -> egui::Response {
        let mut response = ui
            .horizontal(|ui| {
                let (rect, r) = ui.allocate_exact_size(Vec2::splat(24.), egui::Sense::click());
                let base = if r.is_pointer_button_down_on() {
                    &self.check_down
                } else {
                    &self.check_up
                };
                let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1., 1.));
                ui.painter().image(base.id(), rect, uv, Color32::WHITE);
                if *checked {
                    ui.painter()
                        .image(self.check_mark.id(), rect, uv, Color32::WHITE);
                }
                if r.hovered() {
                    // Stock highlight is additive; alpha compositing it hides the check.
                    ui.painter().rect_stroke(rect.shrink(3.),2.,egui::Stroke::new(1.,GOLD),egui::StrokeKind::Inside);
                }
                r.union(ui.add(egui::Label::new(label).sense(egui::Sense::click())))
            })
            .inner;
        if response.clicked() {
            *checked = !*checked;
            response.mark_changed();
        }
        response
    }
    pub fn heading(&self, ui: &mut egui::Ui, text: &str) -> egui::Response {
        ui.heading(egui::RichText::new(text).color(GOLD))
    }
}
fn read_font(chain: &benilla_formats::Chain, data: &Path, name: &str) -> Option<Vec<u8>> {
    chain.read(name).ok().or_else(|| {
        // Loose fonts are supported read-only beside or inside the user's install.
        let file = name.rsplit('\\').next()?;
        [
            data.join("Fonts").join(file),
            data.parent()?.join("Fonts").join(file),
            data.parent()?.join(file),
        ]
        .into_iter()
        .find_map(|p| std::fs::read(p).ok())
    })
}
fn system_cjk_font() -> Option<Vec<u8>> {
    [
        "/System/Library/Fonts/PingFang.ttc",
        "/System/Library/Fonts/STHeiti Light.ttc",
        "C:\\Windows\\Fonts\\msyh.ttc",
        "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
    ]
    .iter()
    .find_map(|p| std::fs::read(p).ok())
}
fn atlas_uv(slice: u8, fraction: f32, horizontal: bool) -> [Pos2; 4] {
    let u = slice as f32 / 8.;
    let v = (slice + 1) as f32 / 8.;
    if horizontal {
        [
            Pos2::new(u, fraction),
            Pos2::new(u, 0.),
            Pos2::new(v, 0.),
            Pos2::new(v, fraction),
        ]
    } else {
        [
            Pos2::new(u, 0.),
            Pos2::new(v, 0.),
            Pos2::new(v, fraction),
            Pos2::new(u, fraction),
        ]
    }
}
fn quad(
    painter: &egui::Painter,
    texture: egui::TextureId,
    rect: Rect,
    uv: [Pos2; 4],
    tint: Color32,
) {
    let mut mesh = egui::Mesh::with_texture(texture);
    for (pos, uv) in [
        rect.left_top(),
        rect.right_top(),
        rect.right_bottom(),
        rect.left_bottom(),
    ]
    .into_iter()
    .zip(uv)
    {
        mesh.vertices.push(egui::epaint::Vertex {
            pos,
            uv,
            color: tint,
        });
    }
    mesh.indices.extend_from_slice(&[0, 1, 2, 0, 2, 3]);
    painter.add(egui::Shape::mesh(mesh));
}
fn tiles(
    painter: &egui::Painter,
    texture: egui::TextureId,
    rect: Rect,
    size: f32,
    uv: impl Fn(Rect, Vec2) -> (Rect, [Pos2; 4]),
) {
    let mut y = rect.top();
    while y < rect.bottom() {
        let mut x = rect.left();
        while x < rect.right() {
            let r = Rect::from_min_size(
                Pos2::new(x, y),
                Vec2::new(size.min(rect.right() - x), size.min(rect.bottom() - y)),
            );
            let (r, uv) = uv(r, r.size() / size);
            quad(painter, texture, r, uv, Color32::WHITE);
            x += size;
        }
        y += size;
    }
}
fn button_three(painter: &egui::Painter, texture: egui::TextureId, rect: Rect, tint: Color32) {
    let cap = (12. * rect.height() / 22.).min(rect.width() / 2.);
    let spans = [
        (rect.left(), rect.left() + cap, 0., 0.09375),
        (rect.left() + cap, rect.right() - cap, 0.09375, 0.53125),
        (rect.right() - cap, rect.right(), 0.53125, 0.625),
    ];
    for (x0, x1, u0, u1) in spans {
        painter.image(
            texture,
            Rect::from_min_max(Pos2::new(x0, rect.top()), Pos2::new(x1, rect.bottom())),
            Rect::from_min_max(Pos2::new(u0, 0.), Pos2::new(u1, 0.6875)),
            tint,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn atlas_edges_use_stock_slices_and_rotate_top() {
        assert_eq!(
            atlas_uv(4, 1.0, false),
            [
                Pos2::new(0.5, 0.0),
                Pos2::new(0.625, 0.0),
                Pos2::new(0.625, 1.0),
                Pos2::new(0.5, 1.0)
            ]
        );
        assert_eq!(
            atlas_uv(2, 0.5, true),
            [
                Pos2::new(0.25, 0.5),
                Pos2::new(0.25, 0.0),
                Pos2::new(0.375, 0.0),
                Pos2::new(0.375, 0.5)
            ]
        );
    }
    #[test]
    fn loads_real_install_art_and_font_when_available() {
        let Some(data) = std::env::var_os("WOW_DATA").filter(|s| !s.is_empty()) else {
            return;
        };
        let context = egui::Context::default();
        let skin =
            WowSkin::load(Path::new(&data), &context).expect("actual WoW skin art and fonts");
        assert_eq!(skin.border.size(), [256, 32]);
        assert_eq!(skin.background.size(), [64, 64]);
        assert_eq!(skin.up.size(), [128, 32]);
        assert!(
            skin.fonts.font_data.contains_key("wow-zh")
                || skin.fonts.font_data.contains_key("wow-latin")
        );
        skin.apply(&context);
        let _ = context.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                skin.paint_frame(ui, ui.max_rect());
                skin.heading(ui, "桌宠设置 / Settings");
                skin.button(ui, "保存并连接");
            });
        });
    }
}

fn slider_value(pointer:f32,left:f32,right:f32)->f32{25.+375.*((pointer-left)/(right-left).max(1.)).clamp(0.,1.)}
#[cfg(test)]mod control_tests {use super::*;#[test]fn scale_track_clicks_clamp_and_interpolate(){assert_eq!(slider_value(-5.,0.,100.),25.);assert_eq!(slider_value(100.,0.,100.),400.);assert_eq!(slider_value(20.,0.,100.),100.);}}

#[cfg(test)]mod interaction_tests {
 use super::*;
 #[test]fn checkbox_hover_preserves_value_and_click_changes_it(){
  let Ok(data)=std::env::var("WOW_DATA")else{return};let ctx=egui::Context::default();let skin=WowSkin::load(Path::new(&data),&ctx).unwrap();let mut checked=true;let mut rect=Rect::NOTHING;
  let mut frame=|events:Vec<egui::Event>,checked:&mut bool|{
   let _=ctx.run(egui::RawInput{events,..Default::default()},|ctx|{egui::CentralPanel::default().show(ctx,|ui|{rect=skin.checkbox(ui,checked,"头盔").rect;});});rect.center()
  };
  let point=frame(vec![],&mut checked);frame(vec![egui::Event::PointerMoved(point)],&mut checked);assert!(checked,"hover must never toggle");
  frame(vec![egui::Event::PointerButton{pos:point,button:egui::PointerButton::Primary,pressed:true,modifiers:Default::default()}],&mut checked);
  frame(vec![egui::Event::PointerButton{pos:point,button:egui::PointerButton::Primary,pressed:false,modifiers:Default::default()}],&mut checked);assert!(!checked,"click toggles once");
 }
}
