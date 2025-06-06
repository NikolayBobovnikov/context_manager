use std::path::{Path, PathBuf};
use std::cmp::Ordering;
use std::fs;
use ignore::{WalkBuilder, DirEntry};
use log::{debug, warn};

use crate::error::{AppError, Result};

#[derive(Debug, Clone)]
pub struct FileNode {
    pub name: String,          // Base name of the file/directory
    pub path: PathBuf,         // Full, canonicalized path
    pub is_dir: bool,
    pub children: Vec<FileNode>, // Sorted: directories first, then files, then alphabetically case-insensitively
}

// Custom sorting for FileNode: directories first, then files, then by name (case-insensitive)
impl PartialEq for FileNode {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path
    }
}

impl Eq for FileNode {}

impl PartialOrd for FileNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for FileNode {
    fn cmp(&self, other: &Self) -> Ordering {
        if self.is_dir && !other.is_dir {
            Ordering::Less
        } else if !self.is_dir && other.is_dir {
            Ordering::Greater
        } else {
            self.name.to_lowercase().cmp(&other.name.to_lowercase())
        }
    }
}

pub struct FileHandler {
    directory: PathBuf,
}

impl FileHandler {
    pub fn new(directory: PathBuf) -> Result<Self> {
        // Validate directory exists and is readable
        if !directory.exists() {
            return Err(AppError::InvalidDirectory(
                format!("Directory does not exist: {:?}", directory)
            ));
        }

        if !directory.is_dir() {
            return Err(AppError::InvalidDirectory(
                format!("Path is not a directory: {:?}", directory)
            ));
        }

        // Test if we can read the directory
        match fs::read_dir(&directory) {
            Ok(_) => {},
            Err(e) => {
                return Err(AppError::PermissionsError {
                    path: directory.clone(),
                    details: format!("Cannot read directory: {}", e),
                });
            }
        }

        Ok(FileHandler { directory })
    }

    pub fn scan_directory(&self, ignore_patterns: Vec<String>) -> Result<FileNode> {
        debug!("Starting directory scan for: {:?}", self.directory);
        
        let mut builder = WalkBuilder::new(&self.directory);
        
        // Configure the walker according to the plan
        builder
            .standard_filters(true)  // respects global gitignore, .git/info/exclude
            .git_global(true)
            .git_ignore(true)
            .git_exclude(true)
            .hidden(false)          // initially include hidden files, let ignore patterns filter them
            .follow_links(false);   // crucial: do not follow symlinks

        // Add additional ignore patterns
        let mut overrides_builder = ignore::overrides::OverrideBuilder::new(&self.directory);
        for pattern_to_ignore in ignore_patterns {
            let blacklist_pattern = format!("!{}", pattern_to_ignore);
            if let Err(e) = overrides_builder.add(&blacklist_pattern) {
                warn!("Failed to add ignore pattern '{}' as blacklist override '{}': {}", pattern_to_ignore, blacklist_pattern, e);
            }
        }
        
        let overrides = overrides_builder.build()
            .map_err(|e| AppError::IgnoreBuild(e))?;
        builder.overrides(overrides);

        let walker = builder.build();
        
        // Build the tree structure
        let root_node = self.build_file_tree(walker)?;
        
        debug!("Directory scan completed");
        Ok(root_node)
    }

    fn build_file_tree(&self, walker: ignore::Walk) -> Result<FileNode> {
        let mut path_to_node: std::collections::HashMap<PathBuf, FileNode> = std::collections::HashMap::new();
        let mut parent_child_map: std::collections::HashMap<PathBuf, Vec<PathBuf>> = std::collections::HashMap::new();

        let mut total_entries = 0;
        let mut processed_entries = 0;

        // First pass: collect all entries and build node relationships
        for result in walker {
            total_entries += 1;
            match result {
                Ok(entry) => {
                    debug!("Processing entry: {:?}", entry.path());
                    if let Err(e) = self.process_dir_entry(entry, &mut path_to_node, &mut parent_child_map) {
                        warn!("Error processing directory entry: {}", e);
                    } else {
                        processed_entries += 1;
                    }
                }
                Err(e) => {
                    warn!("Error walking directory: {}", e);
                }
            }
        }

        debug!("Processed {} out of {} entries", processed_entries, total_entries);
        debug!("Created {} nodes", path_to_node.len());

        // Second pass: build the tree structure
        let root_path = match self.directory.canonicalize() {
            Ok(path) => path,
            Err(e) => {
                return Err(AppError::new_io_error(
                    e,
                    Some(self.directory.clone()),
                    "Failed to canonicalize root directory".to_string(),
                ));
            }
        };

        self.build_tree_recursive(&root_path, &mut path_to_node, &parent_child_map)
    }

    fn process_dir_entry(
        &self,
        entry: DirEntry,
        path_to_node: &mut std::collections::HashMap<PathBuf, FileNode>,
        parent_child_map: &mut std::collections::HashMap<PathBuf, Vec<PathBuf>>,
    ) -> Result<()> {
        let path = entry.path();
        let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
        
        // Canonicalize the path
        let canonical_path = match path.canonicalize() {
            Ok(p) => p,
            Err(e) => {
                debug!("Failed to canonicalize path {:?}: {}", path, e);
                return Ok(()); // Skip this entry
            }
        };

        let name = match path.file_name() {
            Some(name) => name.to_string_lossy().to_string(),
            None => {
                // This is likely the root directory
                match path.file_name() {
                    Some(name) => name.to_string_lossy().to_string(),
                    None => self.directory.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "root".to_string()),
                }
            }
        };

        let node = FileNode {
            name,
            path: canonical_path.clone(),
            is_dir,
            children: Vec::new(),
        };

        path_to_node.insert(canonical_path.clone(), node);

        // Track parent-child relationships
        if let Some(parent_path) = canonical_path.parent() {
            parent_child_map
                .entry(parent_path.to_path_buf())
                .or_insert_with(Vec::new)
                .push(canonical_path);
        }

        Ok(())
    }

    fn build_tree_recursive(
        &self,
        current_path: &Path,
        path_to_node: &mut std::collections::HashMap<PathBuf, FileNode>,
        parent_child_map: &std::collections::HashMap<PathBuf, Vec<PathBuf>>,
    ) -> Result<FileNode> {
        let mut node = path_to_node.remove(current_path)
            .ok_or_else(|| AppError::PathNotFound(current_path.to_path_buf()))?;

        if let Some(children_paths) = parent_child_map.get(current_path) {
            let mut children = Vec::new();
            for child_path in children_paths {
                if let Ok(child_node) = self.build_tree_recursive(child_path, path_to_node, parent_child_map) {
                    children.push(child_node);
                }
            }
            
            // Sort children according to FileNode's Ord implementation
            children.sort();
            node.children = children;
        }

        Ok(node)
    }
} 