use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Instant;
use egui::Context;
use log::{debug, info, warn, error};

use crate::constants::{OUTPUT_FILENAME, UI_STATUS_MESSAGE_DURATION};
use crate::error::Result;
use crate::events::AppEvent;
use crate::file_handler::{FileHandler, FileNode};
use crate::file_monitor::FileMonitor;
use crate::markdown_generator::MarkdownGenerator;
use crate::ui_tree_handler::UITreeHandler;

pub struct MarkdownContextBuilderApp {
    // Core state
    current_directory: Option<PathBuf>,
    root_file_node: Option<FileNode>,
    
    // UI state
    ui_tree_handler: UITreeHandler,
    
    // Communication
    event_sender: mpsc::Sender<AppEvent>,
    event_receiver: mpsc::Receiver<AppEvent>,
    
    // File monitoring
    file_monitor: FileMonitor,
    monitoring_active: bool,
    
    // UI feedback
    status_message: Option<(String, Instant)>,
    error_message: Option<String>,
    
    // Operation states
    is_loading_directory: bool,
    is_generating_markdown: bool,
}

impl MarkdownContextBuilderApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let (event_sender, event_receiver) = mpsc::channel();
        let file_monitor = FileMonitor::new(event_sender.clone());
        
        Self {
            current_directory: None,
            root_file_node: None,
            ui_tree_handler: UITreeHandler::new(),
            event_sender,
            event_receiver,
            file_monitor,
            monitoring_active: false,
            status_message: None,
            error_message: None,
            is_loading_directory: false,
            is_generating_markdown: false,
        }
    }

    fn set_status_message(&mut self, message: String) {
        self.status_message = Some((message, Instant::now()));
        self.error_message = None; // Clear error when showing status
    }

    fn set_error_message(&mut self, message: String) {
        self.error_message = Some(message);
        self.status_message = None; // Clear status when showing error
    }

    fn clear_messages(&mut self) {
        self.status_message = None;
        self.error_message = None;
    }

    fn open_directory_dialog(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            self.open_directory(path);
        }
    }

    fn open_directory(&mut self, directory: PathBuf) {
        info!("Opening directory: {:?}", directory);
        self.is_loading_directory = true;
        self.set_status_message("Scanning directory...".to_string());
        
        // Stop any existing monitoring
        if let Err(e) = self.file_monitor.stop_monitoring() {
            warn!("Error stopping file monitor: {}", e);
        }
        self.monitoring_active = false;
        
        // Clear current state
        self.current_directory = Some(directory.clone());
        self.root_file_node = None;
        self.ui_tree_handler = UITreeHandler::new();
        
        // Start directory scan in background thread
        let sender = self.event_sender.clone();
        thread::spawn(move || {
            let result = FileHandler::new(directory)
                .and_then(|handler| handler.scan_directory());
            
            if let Err(e) = sender.send(AppEvent::DirectoryScanComplete(result)) {
                error!("Failed to send directory scan result: {}", e);
            }
        });
    }

    fn handle_directory_scan_complete(&mut self, result: Result<FileNode>) {
        self.is_loading_directory = false;
        
        match result {
            Ok(root_node) => {
                info!("Directory scan completed successfully");
                self.root_file_node = Some(root_node.clone());
                self.ui_tree_handler.build_from_file_node(&root_node);
                self.set_status_message("Directory loaded successfully".to_string());
            }
            Err(e) => {
                error!("Directory scan failed: {}", e);
                self.set_error_message(format!("Failed to scan directory: {}", e));
                self.current_directory = None;
            }
        }
    }

    fn start_monitoring(&mut self) {
        let selected_files = self.ui_tree_handler.get_selected_files();
        
        if selected_files.is_empty() {
            self.set_error_message("Please select at least one file before starting monitoring".to_string());
            return;
        }

        if self.current_directory.is_some() {
            // First generate the initial markdown
            self.generate_markdown(false);
            
            // Then start file monitoring
            match self.file_monitor.start_monitoring(selected_files.clone()) {
                Ok(()) => {
                    self.monitoring_active = true;
                    self.set_status_message(format!("Monitoring {} files for changes", selected_files.len()));
                }
                Err(e) => {
                    error!("Failed to start file monitoring: {}", e);
                    self.set_error_message(format!("Failed to start monitoring: {}", e));
                }
            }
        }
    }

    fn stop_monitoring(&mut self) {
        match self.file_monitor.stop_monitoring() {
            Ok(()) => {
                self.monitoring_active = false;
                self.set_status_message("File monitoring stopped".to_string());
            }
            Err(e) => {
                error!("Failed to stop file monitoring: {}", e);
                self.set_error_message(format!("Failed to stop monitoring: {}", e));
            }
        }
    }

    fn generate_markdown(&mut self, show_completion_message: bool) {
        if let (Some(directory), Some(root_node)) = (&self.current_directory, &self.root_file_node) {
            let selected_files = self.ui_tree_handler.get_selected_files();
            
            if selected_files.is_empty() {
                if show_completion_message {
                    self.set_error_message("Please select at least one file to generate markdown".to_string());
                }
                return;
            }

            // Clone values before setting status message to avoid borrow issues
            let directory = directory.clone();
            let root_node = root_node.clone();

            self.is_generating_markdown = true;
            if show_completion_message {
                self.set_status_message("Generating markdown...".to_string());
            }
            
            let sender = self.event_sender.clone();
            
            thread::spawn(move || {
                let generator = MarkdownGenerator::new(directory.clone(), selected_files);
                let result = generator.generate_full_markdown(&root_node)
                    .and_then(|content| {
                        let output_path = directory.join(OUTPUT_FILENAME);
                        generator.atomic_write_markdown(&output_path, &content)
                    });
                
                if let Err(e) = sender.send(AppEvent::MarkdownGenerationComplete(result)) {
                    error!("Failed to send markdown generation result: {}", e);
                }
            });
        }
    }

    fn handle_markdown_generation_complete(&mut self, result: Result<()>) {
        self.is_generating_markdown = false;
        
        match result {
            Ok(()) => {
                if let Some(directory) = &self.current_directory {
                    let output_path = directory.join(OUTPUT_FILENAME);
                    self.set_status_message(format!("Markdown generated: {}", output_path.display()));
                }
            }
            Err(e) => {
                error!("Markdown generation failed: {}", e);
                self.set_error_message(format!("Failed to generate markdown: {}", e));
            }
        }
    }

    fn handle_file_modified(&mut self, file_path: PathBuf) {
        debug!("Handling file modification: {:?}", file_path);
        
        if let Some(directory) = &self.current_directory {
            let selected_files = self.ui_tree_handler.get_selected_files();
            
            // Only update if the modified file is in our selection
            if selected_files.contains(&file_path) {
                let directory = directory.clone();
                let sender = self.event_sender.clone();
                
                thread::spawn(move || {
                    let generator = MarkdownGenerator::new(directory.clone(), selected_files);
                    let markdown_path = directory.join(OUTPUT_FILENAME);
                    let result = generator.update_file_section_in_markdown(&markdown_path, &file_path);
                    
                    if let Err(e) = sender.send(AppEvent::PartialMarkdownUpdateComplete(result)) {
                        error!("Failed to send partial markdown update result: {}", e);
                    }
                });
            }
        }
    }

    fn handle_partial_markdown_update_complete(&mut self, result: Result<()>) {
        match result {
            Ok(()) => {
                debug!("Partial markdown update completed successfully");
                // Don't show a message for partial updates to avoid spam
            }
            Err(e) => {
                warn!("Partial markdown update failed: {}", e);
                // Only show error if it's serious
            }
        }
    }

    fn process_events(&mut self) {
        while let Ok(event) = self.event_receiver.try_recv() {
            match event {
                AppEvent::DirectoryScanComplete(result) => {
                    self.handle_directory_scan_complete(result);
                }
                AppEvent::FileModifiedDebounced(file_path) => {
                    self.handle_file_modified(file_path);
                }
                AppEvent::MarkdownGenerationComplete(result) => {
                    self.handle_markdown_generation_complete(result);
                }
                AppEvent::PartialMarkdownUpdateComplete(result) => {
                    self.handle_partial_markdown_update_complete(result);
                }
                AppEvent::WatcherError(error) => {
                    error!("File watcher error: {}", error);
                    self.set_error_message(format!("File watcher error: {}", error));
                    self.monitoring_active = false;
                }
                AppEvent::StatusMessage(message) => {
                    self.set_status_message(message);
                }
                AppEvent::ErrorMessage(message) => {
                    self.set_error_message(message);
                }
            }
        }
    }

    fn render_directory_selection(&mut self, ui: &mut egui::Ui) {
        ui.group(|ui| {
            ui.vertical(|ui| {
                ui.heading("Project Directory");
                ui.add_space(5.0);
                
                ui.horizontal(|ui| {
                    ui.label("Current directory:");
                    ui.add_space(10.0);
                    
                    if let Some(directory) = &self.current_directory {
                        ui.monospace(directory.display().to_string());
                    } else {
                        ui.weak("No directory selected");
                    }
                });
                
                ui.add_space(8.0);
                
                ui.horizontal(|ui| {
                    if ui.add_sized([120.0, 30.0], egui::Button::new("Browse...")).clicked() {
                        self.open_directory_dialog();
                    }
                    
                    if self.current_directory.is_some() {
                        ui.add_space(10.0);
                        if ui.add_sized([100.0, 30.0], egui::Button::new("🔄 Refresh")).clicked() {
                            if let Some(dir) = self.current_directory.clone() {
                                self.open_directory(dir);
                            }
                        }
                    }
                });
            });
        });
    }

    fn render_file_tree(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        
        ui.group(|ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.heading("File Selection");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if self.ui_tree_handler.has_selection() {
                            ui.colored_label(
                                egui::Color32::from_rgb(0, 150, 0), 
                                format!("✓ {} files selected", self.ui_tree_handler.get_selected_files().len())
                            );
                        } else if self.current_directory.is_some() {
                            ui.weak("No files selected");
                        }
                    });
                });
                
                ui.add_space(5.0);
                ui.separator();
                ui.add_space(5.0);
                
                if self.is_loading_directory {
                    ui.vertical_centered(|ui| {
                        ui.add_space(20.0);
                        ui.spinner();
                        ui.add_space(10.0);
                        ui.label("🔍 Scanning directory...");
                        ui.add_space(20.0);
                    });
                } else if self.current_directory.is_some() {
                    egui::ScrollArea::vertical()
                        .max_height(350.0)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                            if self.ui_tree_handler.tree_nodes.is_empty() {
                                ui.vertical_centered(|ui| {
                                    ui.add_space(20.0);
                                    ui.weak("📁 Empty directory or all files filtered");
                                    ui.add_space(20.0);
                                });
                            } else {
                                let selection_changed = self.ui_tree_handler.render_tree(ui);
                                
                                // If monitoring is active and selection changed, restart monitoring
                                if selection_changed && self.monitoring_active {
                                    self.stop_monitoring();
                                    self.start_monitoring();
                                }
                            }
                        });
                } else {
                    ui.vertical_centered(|ui| {
                        ui.add_space(30.0);
                        ui.heading("👆");
                        ui.add_space(10.0);
                        ui.weak("Please select a directory above to view its structure");
                        ui.add_space(30.0);
                    });
                }
            });
        });
    }

    fn render_control_buttons(&mut self, ui: &mut egui::Ui) {
        ui.add_space(10.0);
        
        ui.group(|ui| {
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.heading("Actions");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Monitoring status with better visual indication
                        if self.monitoring_active {
                            ui.colored_label(egui::Color32::from_rgb(0, 150, 0), "🟢 Monitoring Active");
                        } else {
                            ui.colored_label(egui::Color32::from_rgb(150, 150, 150), "⚫ Monitoring Inactive");
                        }
                    });
                });
                
                ui.add_space(5.0);
                ui.separator();
                ui.add_space(8.0);
                
                let has_selection = self.ui_tree_handler.has_selection();
                let can_start = has_selection && !self.monitoring_active && !self.is_generating_markdown;
                let can_stop = self.monitoring_active;
                
                // Action buttons in a more organized layout
                ui.horizontal(|ui| {
                    // Generate markdown button (primary action)
                    let generate_button = egui::Button::new("📝 Generate Markdown")
                        .min_size(egui::vec2(140.0, 35.0));
                    
                    if ui.add_enabled(has_selection && !self.is_generating_markdown, generate_button).clicked() {
                        self.generate_markdown(true);
                    }
                    
                    ui.add_space(10.0);
                    
                    // Start monitoring button
                    let start_button = egui::Button::new("▶️ Start Monitoring")
                        .min_size(egui::vec2(130.0, 35.0));
                    
                    if ui.add_enabled(can_start, start_button).clicked() {
                        self.start_monitoring();
                    }
                    
                    // Stop monitoring button
                    let stop_button = egui::Button::new("⏹️ Stop Monitoring")
                        .min_size(egui::vec2(130.0, 35.0));
                    
                    if ui.add_enabled(can_stop, stop_button).clicked() {
                        self.stop_monitoring();
                    }
                });
                
                ui.add_space(5.0);
                
                // Help text
                if !has_selection && self.current_directory.is_some() {
                    ui.horizontal(|ui| {
                        ui.weak("💡 Select files above to enable actions");
                    });
                } else if self.current_directory.is_none() {
                    ui.horizontal(|ui| {
                        ui.weak("💡 Select a directory first");
                    });
                } else if self.is_generating_markdown {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Generating markdown...");
                    });
                }
            });
        });
    }

    fn render_status_messages(&mut self, ui: &mut egui::Ui) {
        // Clean up expired status messages
        if let Some((_, timestamp)) = &self.status_message {
            if timestamp.elapsed() > UI_STATUS_MESSAGE_DURATION {
                self.status_message = None;
            }
        }
        
        // Show status or error message
        if let Some((message, _)) = &self.status_message {
            let message = message.clone(); // Clone to avoid borrowing issues
            
            ui.add_space(10.0);
            
            egui::Frame::none()
                .fill(egui::Color32::from_rgb(240, 255, 240))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(0, 150, 0)))
                .inner_margin(egui::Margin::same(10.0))
                .rounding(egui::Rounding::same(5.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("✅");
                        ui.colored_label(egui::Color32::from_rgb(0, 120, 0), message);
                    });
                });
        } else if let Some(error_message) = &self.error_message {
            let error_message = error_message.clone(); // Clone to avoid borrowing issues
            
            ui.add_space(10.0);
            
            egui::Frame::none()
                .fill(egui::Color32::from_rgb(255, 240, 240))
                .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(200, 0, 0)))
                .inner_margin(egui::Margin::same(10.0))
                .rounding(egui::Rounding::same(5.0))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label("❌");
                        ui.colored_label(egui::Color32::from_rgb(150, 0, 0), &error_message);
                        
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("✖").clicked() {
                                self.clear_messages();
                            }
                        });
                    });
                });
        }
    }
}

impl eframe::App for MarkdownContextBuilderApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Process background events
        self.process_events();
        
        // Main UI with better layout
        egui::CentralPanel::default().show(ctx, |ui| {
            // Title bar
            ui.vertical_centered(|ui| {
                ui.add_space(10.0);
                ui.heading("🦀 Context Builder - Rust Edition");
                ui.weak("Generate markdown documentation from your project files");
                ui.add_space(10.0);
            });
            
            ui.separator();
            ui.add_space(8.0);
            
            // Main content with proper spacing
            egui::ScrollArea::vertical()
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    self.render_directory_selection(ui);
                    self.render_file_tree(ui);
                    self.render_control_buttons(ui);
                    self.render_status_messages(ui);
                    
                    ui.add_space(20.0); // Bottom padding
                });
        });
        
        // Request repaint for animations (spinner, etc.)
        if self.is_loading_directory || self.is_generating_markdown {
            ctx.request_repaint();
        }
    }
} 