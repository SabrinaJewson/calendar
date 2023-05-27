#![feature(portable_simd)]

fn main() -> anyhow::Result<()> {
    eprintln!("Reading log file…");

    let log = fs::read_to_string("log.toml").context("failed to read `log.toml`")?;
    let log = log.parse::<Log>().context("failed to parse `log.toml`")?;

    eprintln!("Generating PDF…");

    pdf::render(log, "calendar.pdf").context("failed to render PDF")?;

    Ok(())
}

mod pdf {
    pub(crate) fn render(log: Log, file: &str) -> anyhow::Result<()> {
        let document = PdfDocument::empty("Calendar");

        const REGULAR: &str = "/usr/share/fonts/TTF/DejaVuSans.ttf";
        const BOLD: &str = "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf";
        const ITALIC: &str = "/usr/share/fonts/TTF/DejaVuSans-Oblique.ttf";
        let fonts = Fonts {
            regular: Font::new(&document, REGULAR)?,
            bold: Font::new(&document, BOLD)?,
            italic: Font::new(&document, ITALIC)?,
        };

        let mut date = log.start_date();
        let mut days_iter = log.days();
        while days_iter.len() != 0 {
            let year = date.year();

            let mut days = Vec::new();
            let mut past_date = Date::from_ordinal_date(year, 1).unwrap();
            while past_date != date {
                days.push(None);
                past_date = past_date.next_day().unwrap();
            }

            while year == date.year() {
                days.push(days_iter.next().unwrap_or_default());
                date = date.next_day().unwrap();
            }

            assert_eq!(days.len(), usize::from(time::util::days_in_year(year)));
            let mut days = days.into_iter();

            let page_x = Mm(210.0);
            let page_y = Mm(297.0);

            let page = Page::new(&document, (page_x, page_y));

            let title_text = text!(&fonts.bold, "{year}").size(36.0).center();
            let title_vpad = Mm(14.0);
            let y = title_vpad + title_text.height();
            let title_text = title_text.position((page_x / 2.0, y));
            title_text.draw(&page);

            let x_margin = Mm(10.0);
            let x_sep = Mm(10.0);
            let top_margin = y + title_vpad;
            let bottom_margin = title_vpad;
            let col_width = (page_x - x_margin * 2.0 - x_sep * 2.0) / 3.0;
            let row_height = (page_y - top_margin - bottom_margin) / 4.0;
            for month_index in 0..12 {
                let row = month_index / 3;
                let col = month_index % 3;
                let month = Month::try_from(month_index + 1).unwrap();

                let header_padding = Mm(2.0);
                let header_text = text!(&fonts.bold, "{month}").rgb(255, 255, 255).center();
                let left = x_margin + (col_width + x_sep) * f64::from(col);
                let center_line = left + col_width / 2.0;
                let top = top_margin + row_height * f64::from(row);
                let header_y = top + header_padding + header_text.height();
                let header_text = header_text.position((center_line, header_y));

                let bg_height = header_text.height() + header_padding * 2.0;
                draw_rect((left, top, col_width, bg_height), rgb(46, 117, 181), &page);

                header_text.draw(&page);

                let month_starts_on = Date::from_calendar_date(year, month, 1)
                    .unwrap()
                    .monday_based_week();

                let inner_col_width = col_width / 7.0;
                let size = 10.0;
                let vspacing = Mm(2.5);
                for (col, day) in ["M", "T", "W", "T", "F", "S", "S"].into_iter().enumerate() {
                    let text = text!(&fonts.italic, "{day}").size(size).center();
                    let x = left + inner_col_width * col as f64 + inner_col_width / 2.0;
                    let y = top + bg_height + text.height() + vspacing;
                    text.position((x, y)).draw(&page);
                }
                for day in 1..=time::util::days_in_year_month(year, month) {
                    let text = text!(&fonts.regular, "{day}").size(size).center();
                    let date = Date::from_calendar_date(year, month, day).unwrap();
                    let row = date.monday_based_week() - month_starts_on;
                    let col = date.weekday().number_days_from_monday();
                    let left = left + inner_col_width * f64::from(col);
                    let top =
                        top + bg_height + (text.height() + vspacing * 2.0) * f64::from(row + 1);
                    let x = left + inner_col_width / 2.0;
                    let y = top + vspacing + text.height();

                    let highlight = days.next().unwrap().map(|highlight| {
                        (
                            Color::Rgb(Rgb {
                                r: f64::from(highlight.colour.0[0]) / 255.0,
                                g: f64::from(highlight.colour.0[1]) / 255.0,
                                b: f64::from(highlight.colour.0[2]) / 255.0,
                                icc_profile: None,
                            }),
                            highlight.shape,
                        )
                    });
                    match highlight {
                        Some((color, Shape::Circle)) => {
                            let y = y - text.height() / 2.0;
                            let radius = text.height() + Mm(1.0);
                            draw_circle((x, y), radius, 60, color, &page);
                        }
                        Some((color, Shape::Rectangle)) => {
                            // a tiny bit of overlap avoids tiny white bars
                            let width = inner_col_width + Mm(0.1);
                            let height = text.height() + vspacing * 2.0 + Mm(0.1);
                            draw_rect((left, top, width, height), color, &page);
                        }
                        None => {}
                    }

                    text.position((x, y)).draw(&page);
                }
            }
        }

        document
            .check_for_errors()
            .context("error generating PDF")?;

        (|| {
            let mut file = BufWriter::new(fs::File::create(file)?);
            document.save(&mut file)?;
            file.flush()?;
            anyhow::Ok(())
        })()
        .with_context(|| format!("failed to save {file}"))?;

        Ok(())
    }

    struct Page {
        layer: PdfLayerReference,
        y: Mm,
    }

    impl Page {
        fn new(document: &PdfDocumentReference, (x, y): (Mm, Mm)) -> Self {
            let (page, layer) = document.add_page(x, y, "");
            let layer = document.get_page(page).get_layer(layer);
            Self { layer, y }
        }
    }

    struct Fonts {
        regular: Font,
        bold: Font,
        italic: Font,
    }

    struct Font {
        pdf: IndirectFontRef,
        face: rusttype::Font<'static>,
    }

    impl Font {
        fn new(document: &PdfDocumentReference, path: &str) -> anyhow::Result<Self> {
            let file = fs::read(path).with_context(|| format!("failed to open file {path}"))?;
            let pdf = document
                .add_external_font(&*file)
                .with_context(|| format!("failed to load font {path}"))?;
            let face = rusttype::Font::try_from_vec(file).unwrap();
            Ok(Self { pdf, face })
        }
    }

    struct Text<'font> {
        font: &'font Font,
        size: f32,
        color: Color,
        position: (Mm, Mm),
        align: Align,
        content: String,
    }

    macro_rules! text {
        ($font:expr, $($rest:tt)*) => {
            Text {
                font: $font,
                size: 12.0,
                color: rgb(0, 0, 0),
                position: (Mm(0.0), Mm(0.0)),
                align: Align::Left,
                content: format!($($rest)*),
            }
        }
    }
    use text;

    impl Text<'_> {
        fn size(mut self, size: f32) -> Self {
            self.size = size;
            self
        }
        fn rgb(mut self, r: u8, g: u8, b: u8) -> Self {
            self.color = rgb(r, g, b);
            self
        }
        fn center(mut self) -> Self {
            self.align = Align::Center;
            self
        }
        fn position(mut self, position: (Mm, Mm)) -> Self {
            self.position = position;
            self
        }
        fn scale(&self) -> rusttype::Scale {
            let metrics = self.font.face.v_metrics_unscaled();
            let units_per_em = f32::from(self.font.face.units_per_em());
            let glyph_height = (metrics.ascent - metrics.descent) / units_per_em;
            rusttype::Scale::uniform(glyph_height * self.size)
        }
        fn height(&self) -> Mm {
            let metrics = self.font.face.v_metrics(self.scale());
            to_mm(metrics.ascent + metrics.descent)
        }
        fn width(&self) -> Mm {
            let scale = self.scale();

            let mut width = 0.0;
            let mut last_glyph = None;
            for glyph in self.font.face.glyphs_for(self.content.chars()) {
                let glyph = glyph.scaled(scale);
                if let &Some(last_glyph) = &last_glyph {
                    width += self.font.face.pair_kerning(scale, last_glyph, glyph.id());
                }
                width += glyph.h_metrics().advance_width;
                last_glyph = Some(glyph.id());
            }

            to_mm(width)
        }
        fn draw(self, page: &Page) {
            let shift_left = match self.align {
                Align::Left => Mm(0.0),
                Align::Center => self.width() / 2.0,
            };
            let x = self.position.0 - shift_left;
            let y = page.y - self.position.1;

            page.layer.begin_text_section();
            page.layer.set_fill_color(self.color);
            page.layer.set_font(&self.font.pdf, f64::from(self.size));
            page.layer.set_text_cursor(x, y);
            page.layer.write_text(self.content, &self.font.pdf);
            page.layer.end_text_section();
        }
    }

    fn draw_rect((left, top, width, height): (Mm, Mm, Mm, Mm), color: Color, page: &Page) {
        page.layer.set_fill_color(color);
        page.layer.add_shape(Line {
            points: vec![
                (Point::new(left, page.y - top), false),
                (Point::new(left + width, page.y - top), false),
                (Point::new(left + width, page.y - (top + height)), false),
                (Point::new(left, page.y - (top + height)), false),
            ],
            is_closed: true,
            has_fill: true,
            has_stroke: false,
            is_clipping_path: false,
        });
    }

    fn draw_circle((x, y): (Mm, Mm), radius: Mm, points: u32, color: Color, page: &Page) {
        page.layer.set_fill_color(color);
        page.layer.add_shape(Line {
            points: (0..points)
                .map(|i| {
                    let angle = f64::from(i) / f64::from(points) * f64::consts::TAU;
                    let x = x + radius * angle.cos();
                    let y = y - radius * angle.sin();
                    (Point::new(x, page.y - y), false)
                })
                .collect(),
            is_closed: true,
            has_fill: true,
            has_stroke: false,
            is_clipping_path: false,
        });
    }

    fn to_mm(pt: f32) -> Mm {
        Mm::from(Pt(f64::from(pt)))
    }

    fn rgb(r: u8, g: u8, b: u8) -> Color {
        Color::Rgb(Rgb::new(
            f64::from(r) / 255.0,
            f64::from(g) / 255.0,
            f64::from(b) / 255.0,
            None,
        ))
    }

    enum Align {
        Left,
        Center,
    }

    use crate::log::Log;
    use crate::log::Shape;
    use anyhow::Context as _;
    use printpdf::Color;
    use printpdf::IndirectFontRef;
    use printpdf::Line;
    use printpdf::Mm;
    use printpdf::PdfDocument;
    use printpdf::PdfDocumentReference;
    use printpdf::PdfLayerReference;
    use printpdf::Point;
    use printpdf::Pt;
    use printpdf::Rgb;
    use std::f64;
    use std::fs;
    use std::io::BufWriter;
    use std::io::Write;
    use time::Date;
    use time::Month;
}

use log::Log;
mod log;

use anyhow::Context as _;
use std::fs;
