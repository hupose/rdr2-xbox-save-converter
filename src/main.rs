use eframe::egui;
use rdr2_xbox_save_converter::{
    convert_archive, create_config_template, default_config_path, load_xbox_slots,
};
use std::path::Path;

const XBOX_KEY_SOURCE: &str =
    "https://community.wemod.com/t/gta-v-save-block-editor-0-0-3-x360-source/2899";
const PC_KEY_SOURCE: &str = "https://github.com/hzhreal/HTOS/blob/31fca60508251e591afe261b927d73c76a7b3503/data/crypto/rstar_crypt.py";

struct ConverterApp {
    input_zip: String,
    output_dir: String,
    config_path: String,
    status: String,
}

impl Default for ConverterApp {
    fn default() -> Self {
        Self {
            input_zip: String::new(),
            output_dir: String::new(),
            config_path: default_config_path().display().to_string(),
            status: "选择 xbcsmgrrev 导出的 ZIP，然后先检查存档。程序完全离线运行。".into(),
        }
    }
}

impl ConverterApp {
    fn select_input(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("ZIP archive", &["zip"])
            .pick_file()
        {
            self.input_zip = path.display().to_string();
            if let Some(parent) = path.parent() {
                self.output_dir = parent.join("converted_pc_saves").display().to_string();
            }
        }
    }

    fn select_output_parent(&mut self) {
        if let Some(parent) = rfd::FileDialog::new().pick_folder() {
            self.output_dir = parent.join("converted_pc_saves").display().to_string();
        }
    }

    fn select_config(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("TOML config", &["toml"])
            .pick_file()
        {
            self.config_path = path.display().to_string();
        }
    }

    fn create_template(&mut self) {
        match create_config_template(Path::new(&self.config_path)) {
            Ok(()) => {
                self.status = format!(
                    "已创建配置模板：{}\n请按下方公开出处填写两把 key；不要把填好的文件上传。",
                    self.config_path
                );
            }
            Err(error) => self.status = format!("未创建配置：{error}"),
        }
    }

    fn inspect(&mut self) {
        if self.input_zip.trim().is_empty() {
            self.status = "请先选择输入 ZIP。".into();
            return;
        }
        match load_xbox_slots(Path::new(&self.input_zip)) {
            Ok(slots) => {
                let mut lines = vec![format!("检查通过：找到 {} 个剧情存档。", slots.len())];
                for slot in slots {
                    lines.push(format!(
                        "{} → {} | {} | {} bytes",
                        slot.xbox_name,
                        slot.pc_name,
                        slot.title,
                        slot.data.len()
                    ));
                }
                self.status = lines.join("\n");
            }
            Err(error) => self.status = format!("检查失败：{error}"),
        }
    }

    fn convert(&mut self) {
        if self.input_zip.trim().is_empty()
            || self.output_dir.trim().is_empty()
            || self.config_path.trim().is_empty()
        {
            self.status = "输入 ZIP、输出目录和 key 配置路径都不能为空。".into();
            return;
        }
        match convert_archive(
            Path::new(&self.input_zip),
            Path::new(&self.output_dir),
            Path::new(&self.config_path),
        ) {
            Ok(manifest) => {
                self.status = format!(
                    "转换成功：{} 个 PC 存档已写入 {}\n每份均通过 RSAV、10 个 CHKS、Xbox/PC AES 往返及主体一致性检查。",
                    manifest.len(),
                    self.output_dir
                );
            }
            Err(error) => self.status = format!("拒绝转换：{error}"),
        }
    }
}

impl eframe::App for ConverterApp {
    fn update(&mut self, context: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(context, |ui| {
            ui.heading("RDR2 Xbox → PC 存档转换器");
            ui.label("本地离线转换；不登录账号、不联网、不上传存档，也不在程序中内置游戏密钥。");
            ui.add_space(10.0);

            egui::Grid::new("paths")
                .num_columns(3)
                .spacing([8.0, 8.0])
                .show(ui, |ui| {
                    ui.label("Xbox ZIP");
                    ui.text_edit_singleline(&mut self.input_zip);
                    if ui.button("选择…").clicked() {
                        self.select_input();
                    }
                    ui.end_row();

                    ui.label("输出目录");
                    ui.text_edit_singleline(&mut self.output_dir);
                    if ui.button("选择父目录…").clicked() {
                        self.select_output_parent();
                    }
                    ui.end_row();

                    ui.label("key 配置");
                    ui.text_edit_singleline(&mut self.config_path);
                    if ui.button("选择…").clicked() {
                        self.select_config();
                    }
                    ui.end_row();
                });

            ui.horizontal(|ui| {
                if ui.button("创建空白配置模板").clicked() {
                    self.create_template();
                }
                if ui.button("检查存档").clicked() {
                    self.inspect();
                }
                if ui.button("转换并严格验证").clicked() {
                    self.convert();
                }
            });

            ui.separator();
            ui.label("密钥公开出处（链接页面中查找指定字段；本项目不复制 key）：");
            ui.horizontal_wrapped(|ui| {
                ui.label("Xbox：2013 帖子中的 GTAV= 行");
                ui.hyperlink_to("打开来源", XBOX_KEY_SOURCE);
                ui.label("　PC：源码中的 PC_KEY");
                ui.hyperlink_to("打开来源", PC_KEY_SOURCE);
            });

            ui.separator();
            ui.label("状态");
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .max_height(300.0)
                .show(ui, |ui| {
                    ui.add(
                        egui::TextEdit::multiline(&mut self.status)
                            .desired_width(f32::INFINITY)
                            .interactive(false),
                    );
                });
            ui.add_space(6.0);
            ui.small("安全策略：输出目录必须不存在，源 ZIP 永不修改；任何结构、校验或往返失败都会停止且不生成最终目录。");
        });
    }
}

fn run_cli(arguments: &[String]) -> Option<i32> {
    if arguments.first().map(String::as_str) != Some("--convert") {
        return None;
    }
    if arguments.len() != 4 {
        eprintln!("usage: rdr2-xbox-save-converter --convert INPUT.zip OUTPUT_DIR KEYS.toml");
        return Some(2);
    }
    match convert_archive(
        Path::new(&arguments[1]),
        Path::new(&arguments[2]),
        Path::new(&arguments[3]),
    ) {
        Ok(manifest) => {
            println!("converted and verified {} save files", manifest.len());
            Some(0)
        }
        Err(error) => {
            eprintln!("conversion refused: {error}");
            Some(2)
        }
    }
}

fn main() -> eframe::Result {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if let Some(code) = run_cli(&arguments) {
        std::process::exit(code);
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([920.0, 640.0])
            .with_min_inner_size([720.0, 500.0]),
        ..Default::default()
    };
    eframe::run_native(
        "RDR2 Xbox Save Converter",
        options,
        Box::new(|_creation_context| Ok(Box::<ConverterApp>::default())),
    )
}
