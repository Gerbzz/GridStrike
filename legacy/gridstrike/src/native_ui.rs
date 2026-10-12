//! Native grid editor over the same tested workbook and `.gstrike` store.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use eframe::egui;
use uuid::Uuid;

use crate::{CellAddr, CellInput, MAX_COLS, MAX_ROWS, Workbook};

pub struct NativeEditor {
    path: PathBuf,
    workbook: Workbook,
    sheet: Uuid,
    selected: CellAddr,
    address_draft: String,
    cell_draft: String,
    literal_text: bool,
    draft_dirty: bool,
    workbook_dirty: bool,
    sheet_name_draft: String,
    export_path_draft: String,
    row_origin: u32,
    col_origin: u16,
    status: String,
}

impl NativeEditor {
    pub fn open_or_create(path: PathBuf) -> Result<Self, String> {
        let workbook = if path.exists() {
            Workbook::open(&path)?
        } else {
            let workbook = Workbook::new("Sheet 1");
            workbook.save_new(&path)?;
            workbook
        };
        let sheet = workbook.active_sheet();
        let selected = CellAddr { row: 1, col: 0 };
        let export_path_draft = path.with_extension("csv").display().to_string();
        let mut editor = Self {
            path,
            workbook,
            sheet,
            selected,
            address_draft: "A1".into(),
            cell_draft: String::new(),
            literal_text: false,
            draft_dirty: false,
            workbook_dirty: false,
            sheet_name_draft: String::new(),
            export_path_draft,
            row_origin: 1,
            col_origin: 0,
            status: "Ready".into(),
        };
        editor.refresh_draft();
        Ok(editor)
    }

    fn refresh_draft(&mut self) {
        let input = self.workbook.cell(self.sheet, self.selected);
        self.cell_draft = match input {
            None | Some(CellInput::Empty) => String::new(),
            Some(CellInput::Text(text) | CellInput::Formula(text)) => text.clone(),
            Some(CellInput::Number(number)) => number.to_string(),
        };
        self.literal_text = matches!(input, Some(CellInput::Text(_)));
        self.draft_dirty = false;
        self.address_draft = self.selected.a1();
    }

    pub fn select_address(&mut self, source: &str) -> Result<(), String> {
        if self.draft_dirty {
            return Err("Apply or discard the current cell edit before moving.".into());
        }
        let address = CellAddr::parse(source)?;
        self.selected = address;
        self.row_origin = address.row.saturating_sub(2).max(1);
        self.col_origin = address.col.saturating_sub(2);
        self.refresh_draft();
        Ok(())
    }

    pub fn set_draft(&mut self, source: &str) {
        self.cell_draft = source.into();
        self.draft_dirty = true;
    }

    pub fn set_text_literal(&mut self, literal: bool) {
        self.literal_text = literal;
        self.draft_dirty = true;
    }

    pub fn commit_edit(&mut self) -> Result<(), String> {
        let input = if self.literal_text {
            CellInput::Text(self.cell_draft.clone())
        } else if self.cell_draft.is_empty() {
            CellInput::Empty
        } else if self.cell_draft.starts_with('=') {
            CellInput::Formula(self.cell_draft.clone())
        } else if let Ok(number) = self.cell_draft.parse::<f64>() {
            CellInput::Number(number)
        } else {
            CellInput::Text(self.cell_draft.clone())
        };
        self.workbook.set(self.sheet, self.selected, input)?;
        self.draft_dirty = false;
        self.workbook_dirty = true;
        self.status = format!(
            "Applied {}. Save the workbook to keep it.",
            self.selected.a1()
        );
        Ok(())
    }

    pub fn display_value(&self, address: &str) -> Result<String, String> {
        Ok(self
            .workbook
            .value(self.sheet, CellAddr::parse(address)?)
            .to_string())
    }

    pub fn save(&mut self) -> Result<(), String> {
        if self.draft_dirty {
            return Err("Apply or discard the current cell edit before saving.".into());
        }
        self.workbook.save(&self.path)?;
        self.workbook_dirty = false;
        self.status = "Saved to the local workbook.".into();
        Ok(())
    }

    pub fn undo(&mut self) -> Result<(), String> {
        if self.draft_dirty {
            return Err("Apply or discard the current cell edit before undo.".into());
        }
        self.workbook.undo()?;
        self.workbook_dirty = true;
        self.refresh_draft();
        self.status = "Undone. Save the workbook to keep this state.".into();
        Ok(())
    }

    pub fn redo(&mut self) -> Result<(), String> {
        if self.draft_dirty {
            return Err("Apply or discard the current cell edit before redo.".into());
        }
        self.workbook.redo()?;
        self.workbook_dirty = true;
        self.refresh_draft();
        self.status = "Redone. Save the workbook to keep this state.".into();
        Ok(())
    }

    pub fn export_csv(&self, path: &Path) -> Result<(), String> {
        let csv = self.workbook.export_csv(self.sheet)?;
        let parent = path.parent().ok_or("CSV path has no parent")?;
        if !parent.is_dir() {
            return Err("CSV parent directory does not exist".into());
        }
        let staged = path.with_extension(format!("csv.{}.tmp", Uuid::new_v4()));
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staged)
            .map_err(|e| e.to_string())?;
        let written = file
            .write_all(csv.as_bytes())
            .and_then(|()| file.sync_all());
        drop(file);
        if let Err(error) = written {
            let _ = fs::remove_file(&staged);
            return Err(error.to_string());
        }
        let result = fs::hard_link(&staged, path)
            .map_err(|e| format!("CSV destination exists or cannot be written: {e}"));
        let _ = fs::remove_file(&staged);
        result
    }

    fn show_error(&mut self, result: Result<(), String>) {
        if let Err(error) = result {
            self.status = error;
        }
    }

    fn sheet_rail(&mut self, ui: &mut egui::Ui) {
        ui.heading("Sheets");
        ui.label(egui::RichText::new("Local workbook").weak());
        ui.separator();
        let sheets: Vec<_> = self
            .workbook
            .sheets()
            .iter()
            .map(|sheet| (sheet.id, sheet.name.clone()))
            .collect();
        for (id, name) in sheets {
            if ui.selectable_label(self.sheet == id, name).clicked() {
                if self.draft_dirty {
                    self.status =
                        "Apply or discard the current cell edit before changing sheets.".into();
                } else {
                    self.sheet = id;
                    let result = self.workbook.select_sheet(id);
                    self.show_error(result);
                    self.selected = CellAddr { row: 1, col: 0 };
                    self.row_origin = 1;
                    self.col_origin = 0;
                    self.refresh_draft();
                    self.workbook_dirty = true;
                }
            }
        }
        ui.separator();
        ui.add(egui::TextEdit::singleline(&mut self.sheet_name_draft).hint_text("New sheet name"));
        if ui.button("Add sheet").clicked() {
            if self.draft_dirty {
                self.status =
                    "Apply or discard the current cell edit before adding a sheet.".into();
            } else {
                match self.workbook.add_sheet(&self.sheet_name_draft) {
                    Ok(id) => {
                        self.sheet = id;
                        self.selected = CellAddr { row: 1, col: 0 };
                        self.row_origin = 1;
                        self.col_origin = 0;
                        self.sheet_name_draft.clear();
                        self.refresh_draft();
                        self.workbook_dirty = true;
                        self.status = "Sheet added. Save the workbook to keep it.".into();
                    }
                    Err(error) => self.status = error,
                }
            }
        }
    }

    fn formula_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Cell");
            let address = ui.add_sized(
                [92.0, 28.0],
                egui::TextEdit::singleline(&mut self.address_draft),
            );
            if address.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                let candidate = self.address_draft.clone();
                let result = self.select_address(&candidate);
                self.show_error(result);
            }
            if ui.button("Go").clicked() {
                let candidate = self.address_draft.clone();
                let result = self.select_address(&candidate);
                self.show_error(result);
            }
            ui.separator();
            ui.label("Value or formula");
            if ui
                .add(egui::TextEdit::singleline(&mut self.cell_draft).desired_width(f32::INFINITY))
                .changed()
            {
                self.draft_dirty = true;
            }
        });
        ui.horizontal(|ui| {
            if ui
                .checkbox(&mut self.literal_text, "Treat as text")
                .changed()
            {
                self.draft_dirty = true;
            }
            if ui
                .add_enabled(self.draft_dirty, egui::Button::new("Apply cell"))
                .clicked()
            {
                let result = self.commit_edit();
                self.show_error(result);
            }
            if self.draft_dirty && ui.button("Discard draft").clicked() {
                self.refresh_draft();
                self.status = "Cell draft discarded.".into();
            }
            ui.separator();
            ui.label(format!(
                "Displayed: {}",
                self.workbook.value(self.sheet, self.selected)
            ));
        });
    }

    fn grid(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("gridstrike_cells")
                    .striped(true)
                    .spacing([8.0, 6.0])
                    .show(ui, |ui| {
                        ui.strong("");
                        for offset in 0..10u16 {
                            let col = self.col_origin + offset;
                            if col >= MAX_COLS {
                                break;
                            }
                            ui.strong(CellAddr { row: 1, col }.a1().trim_end_matches('1'));
                        }
                        ui.end_row();
                        for offset in 0..30u32 {
                            let row = self.row_origin + offset;
                            if row > MAX_ROWS {
                                break;
                            }
                            ui.strong(row.to_string());
                            for col_offset in 0..10u16 {
                                let col = self.col_origin + col_offset;
                                if col >= MAX_COLS {
                                    break;
                                }
                                let address = CellAddr { row, col };
                                let mut label =
                                    self.workbook.value(self.sheet, address).to_string();
                                if label.chars().count() > 18 {
                                    label =
                                        format!("{}…", label.chars().take(17).collect::<String>());
                                }
                                let label = if label.is_empty() { " ".into() } else { label };
                                if ui
                                    .add_sized(
                                        [118.0, 26.0],
                                        egui::Button::selectable(self.selected == address, label),
                                    )
                                    .clicked()
                                {
                                    let result = self.select_address(&address.a1());
                                    self.show_error(result);
                                }
                            }
                            ui.end_row();
                        }
                    });
            });
    }
}

impl eframe::App for NativeEditor {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let style = ui.style_mut();
        style.spacing.item_spacing = egui::vec2(9.0, 8.0);
        style.visuals.selection.bg_fill = egui::Color32::from_rgb(23, 99, 202);
        if ui.input(|input| input.modifiers.command && input.key_pressed(egui::Key::S)) {
            let result = self.save();
            self.show_error(result);
        }
        egui::Panel::top("gridstrike_toolbar")
            .exact_size(86.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("GridStrike");
                    ui.separator();
                    ui.label(
                        self.path
                            .file_name()
                            .map(|name| name.to_string_lossy())
                            .unwrap_or_default(),
                    );
                    if self.workbook_dirty || self.draft_dirty {
                        ui.strong("Unsaved");
                    }
                    if ui.button("Save (Ctrl+S)").clicked() {
                        let result = self.save();
                        self.show_error(result);
                    }
                    if ui.button("Undo").clicked() {
                        let result = self.undo();
                        self.show_error(result);
                    }
                    if ui.button("Redo").clicked() {
                        let result = self.redo();
                        self.show_error(result);
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("CSV path");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.export_path_draft)
                            .desired_width(390.0),
                    );
                    if ui.button("Export safe CSV").clicked() {
                        match self.export_csv(Path::new(&self.export_path_draft)) {
                            Ok(()) => {
                                self.status =
                                    "CSV exported without replacing an existing file.".into()
                            }
                            Err(error) => self.status = error,
                        }
                    }
                    ui.label(egui::RichText::new(&self.status).small());
                });
            });
        egui::Panel::left("gridstrike_sheets")
            .exact_size(190.0)
            .show(ui, |ui| self.sheet_rail(ui));
        egui::CentralPanel::default().show(ui, |ui| {
            self.formula_bar(ui);
            ui.separator();
            self.grid(ui);
        });
    }
}
