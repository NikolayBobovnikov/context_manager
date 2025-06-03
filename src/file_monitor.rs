use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use log::{debug, info, error};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::constants::DEBOUNCE_DURATION;
use crate::error::{AppError, Result};
use crate::events::AppEvent;

pub struct FileMonitor {
    watcher: Option<RecommendedWatcher>,
    event_sender: mpsc::Sender<AppEvent>,
    debounce_map: HashMap<PathBuf, Instant>,
    debounce_thread_handle: Option<thread::JoinHandle<()>>,
    stop_debounce_sender: Option<mpsc::Sender<()>>,
}

impl FileMonitor {
    pub fn new(event_sender: mpsc::Sender<AppEvent>) -> Self {
        Self {
            watcher: None,
            event_sender,
            debounce_map: HashMap::new(),
            debounce_thread_handle: None,
            stop_debounce_sender: None,
        }
    }

    pub fn start_monitoring(&mut self, files_to_watch: Vec<PathBuf>) -> Result<()> {
        // Stop any existing monitoring
        self.stop_monitoring()?;

        if files_to_watch.is_empty() {
            return Ok(());
        }

        info!("Starting file monitoring for {} files", files_to_watch.len());

        // Create a channel for file events
        let (file_event_sender, file_event_receiver) = mpsc::channel();
        
        // Clone the event sender for the debounce thread
        let app_event_sender = self.event_sender.clone();
        
        // Create debounce thread
        let (stop_sender, stop_receiver) = mpsc::channel();
        self.stop_debounce_sender = Some(stop_sender);
        
        let debounce_handle = thread::spawn(move || {
            Self::debounce_thread(file_event_receiver, app_event_sender, stop_receiver);
        });
        self.debounce_thread_handle = Some(debounce_handle);

        // Create the file watcher
        let mut watcher = notify::recommended_watcher(move |result: notify::Result<Event>| {
            match result {
                Ok(event) => {
                    if let Err(e) = file_event_sender.send(event) {
                        error!("Failed to send file event: {}", e);
                    }
                }
                Err(e) => {
                    error!("File watcher error: {}", e);
                }
            }
        }).map_err(AppError::Notify)?;

        // Watch the parent directories of each file
        let mut watched_dirs = std::collections::HashSet::new();
        for file_path in &files_to_watch {
            if let Some(parent_dir) = file_path.parent() {
                if watched_dirs.insert(parent_dir.to_path_buf()) {
                    watcher.watch(parent_dir, RecursiveMode::NonRecursive)
                        .map_err(AppError::Notify)?;
                    debug!("Watching directory: {:?}", parent_dir);
                }
            }
        }

        self.watcher = Some(watcher);
        info!("File monitoring started successfully");
        Ok(())
    }

    pub fn stop_monitoring(&mut self) -> Result<()> {
        info!("Stopping file monitoring");

        // Stop the watcher
        if let Some(watcher) = self.watcher.take() {
            // The watcher will be dropped, which stops it
            drop(watcher);
        }

        // Stop the debounce thread
        if let Some(stop_sender) = self.stop_debounce_sender.take() {
            let _ = stop_sender.send(()); // Ignore errors, thread might already be stopped
        }

        if let Some(handle) = self.debounce_thread_handle.take() {
            let _ = handle.join(); // Ignore errors
        }

        self.debounce_map.clear();
        info!("File monitoring stopped");
        Ok(())
    }

    fn debounce_thread(
        file_event_receiver: mpsc::Receiver<Event>,
        app_event_sender: mpsc::Sender<AppEvent>,
        stop_receiver: mpsc::Receiver<()>,
    ) {
        let mut debounce_map: HashMap<PathBuf, Instant> = HashMap::new();
        let mut last_check = Instant::now();

        loop {
            // Check for stop signal (non-blocking)
            if stop_receiver.try_recv().is_ok() {
                debug!("Debounce thread received stop signal");
                break;
            }

            // Process incoming file events (non-blocking)
            while let Ok(event) = file_event_receiver.try_recv() {
                if let Some(file_path) = Self::extract_relevant_file_path(&event) {
                    debug!("File event for: {:?}", file_path);
                    debounce_map.insert(file_path, Instant::now());
                }
            }

            // Check for debounced events (every 100ms)
            let now = Instant::now();
            if now.duration_since(last_check) >= Duration::from_millis(100) {
                let mut to_send = Vec::new();
                debounce_map.retain(|path, timestamp| {
                    if now.duration_since(*timestamp) >= DEBOUNCE_DURATION {
                        to_send.push(path.clone());
                        false // Remove from map
                    } else {
                        true // Keep in map
                    }
                });

                // Send debounced events
                for path in to_send {
                    debug!("Sending debounced file event for: {:?}", path);
                    if let Err(e) = app_event_sender.send(AppEvent::FileModifiedDebounced(path)) {
                        error!("Failed to send debounced file event: {}", e);
                        break; // Channel is closed, stop the thread
                    }
                }

                last_check = now;
            }

            // Small sleep to prevent busy waiting
            thread::sleep(Duration::from_millis(50));
        }

        debug!("Debounce thread exiting");
    }

    fn extract_relevant_file_path(event: &Event) -> Option<PathBuf> {
        // We're interested in modify events
        match &event.kind {
            EventKind::Modify(_) => {
                // Take the first path from the event
                event.paths.first().map(|p| p.to_path_buf())
            }
            _ => None,
        }
    }

    #[allow(dead_code)]
    pub fn is_monitoring(&self) -> bool {
        self.watcher.is_some()
    }
}

impl Drop for FileMonitor {
    fn drop(&mut self) {
        let _ = self.stop_monitoring(); // Ignore errors during drop
    }
} 