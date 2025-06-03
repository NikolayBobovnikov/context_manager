use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use log::{debug, warn};

use crate::constants::{
    MARKDOWN_HEADER_CONTEXT, MARKDOWN_HEADER_STRUCTURE, MARKDOWN_HEADER_FILES, MARKDOWN_CODE_BLOCK
};
use crate::error::{AppError, Result};
use crate::file_handler::FileNode;

pub struct MarkdownGenerator {
    directory: PathBuf,
    selected_files: HashSet<PathBuf>,
}

impl MarkdownGenerator {
    pub fn new(directory: PathBuf, selected_files: Vec<PathBuf>) -> Self {
        Self {
            directory,
            selected_files: selected_files.into_iter().collect(),
        }
    }

    pub fn generate_full_markdown(&self, root_node: &FileNode) -> Result<String> {
        debug!("Generating full markdown for {} selected files", self.selected_files.len());
        
        let mut content = String::new();
        
        // Context header
        content.push_str(&format!("{}\n\n", MARKDOWN_HEADER_CONTEXT));
        
        // Project structure section
        content.push_str(&self.generate_structure_section(root_node)?);
        content.push_str("\n\n");
        
        // Files section
        content.push_str(&self.generate_files_section()?);
        
        Ok(content)
    }

    pub fn generate_structure_section(&self, root_node: &FileNode) -> Result<String> {
        let mut structure_content = String::new();
        structure_content.push_str(&format!("{}\n", MARKDOWN_HEADER_STRUCTURE));
        structure_content.push_str(&format!("{}\n", MARKDOWN_CODE_BLOCK));
        
        let mut structure_lines = String::new();
        let mut is_last_child_stack = Vec::new();
        
        self.build_structure_string_recursive(
            root_node,
            &self.directory,
            &Path::new(""),
            0,
            &mut is_last_child_stack,
            &mut structure_lines,
        )?;
        
        structure_content.push_str(&structure_lines);
        structure_content.push_str(&format!("{}", MARKDOWN_CODE_BLOCK));
        
        Ok(structure_content)
    }

    pub fn generate_files_section(&self) -> Result<String> {
        let mut content = String::new();
        content.push_str(&format!("{}\n\n", MARKDOWN_HEADER_FILES));
        
        // Sort selected files for consistent output
        let mut sorted_files: Vec<_> = self.selected_files.iter().collect();
        sorted_files.sort();
        
        for (i, file_path) in sorted_files.iter().enumerate() {
            if i > 0 {
                content.push_str("\n\n");
            }
            content.push_str(&self.generate_file_section(file_path)?);
        }
        
        Ok(content)
    }

    pub fn generate_file_section(&self, file_path: &Path) -> Result<String> {
        let relative_path = file_path.strip_prefix(&self.directory)
            .map_err(|_| AppError::StripPrefixError {
                prefix: self.directory.clone(),
                path: file_path.to_path_buf(),
            })?;
        
        // Use forward slashes for cross-platform consistency
        let display_path = relative_path.to_string_lossy().replace('\\', "/");
        let extension = self.get_file_extension(file_path);
        let content = self.read_file_content(file_path)?;
        
        Ok(format!(
            "### {}\n\n{}{}\n{}\n{}",
            display_path,
            MARKDOWN_CODE_BLOCK,
            extension,
            content,
            MARKDOWN_CODE_BLOCK
        ))
    }

    fn build_structure_string_recursive(
        &self,
        node: &FileNode,
        base_dir_path: &Path,
        current_relative_path: &Path,
        depth: usize,
        is_last_child_stack: &mut Vec<bool>,
        output: &mut String,
    ) -> Result<()> {
        if depth == 0 {
            // Root directory
            let root_name = base_dir_path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "root".to_string());
            output.push_str(&format!("{}\n", root_name));
        } else {
            let prefix = self.get_branch_prefix(depth, is_last_child_stack);
            if node.is_dir {
                output.push_str(&format!("{}├── {}/\n", prefix, node.name));
            } else {
                output.push_str(&format!("{}├── {}\n", prefix, node.name));
            }
        }

        if node.is_dir {
            // Filter children: only include directories that contain selected files, or selected files themselves
            let children_to_render: Vec<&FileNode> = node.children.iter()
                .filter(|child_node| {
                    self.selected_files.contains(&child_node.path) ||
                    (child_node.is_dir && self.directory_contains_selected_file(child_node))
                })
                .collect();

            let num_children_to_render = children_to_render.len();
            for (i, child) in children_to_render.iter().enumerate() {
                is_last_child_stack.push(i == num_children_to_render - 1);
                let child_relative_path = current_relative_path.join(&child.name);
                self.build_structure_string_recursive(
                    child,
                    base_dir_path,
                    &child_relative_path,
                    depth + 1,
                    is_last_child_stack,
                    output,
                )?;
                is_last_child_stack.pop();
            }
        }

        Ok(())
    }

    fn get_branch_prefix(&self, depth: usize, is_last_child_stack: &[bool]) -> String {
        let mut prefix = String::new();
        if depth > 1 {
            for i in 1..depth.saturating_sub(1) {
                prefix.push_str(if is_last_child_stack.get(i).copied().unwrap_or(false) { 
                    "    " 
                } else { 
                    "│   " 
                });
            }
            
            if depth > 1 {
                if is_last_child_stack.get(depth - 1).copied().unwrap_or(false) {
                    prefix.push_str("└── ");
                } else {
                    prefix.push_str("├── ");
                }
            }
        } else if depth == 1 {
            // Direct children of root
            prefix.push_str("│   ");
        }
        prefix
    }

    fn directory_contains_selected_file(&self, dir_node: &FileNode) -> bool {
        if !dir_node.is_dir {
            return false;
        }
        
        for child in &dir_node.children {
            if self.selected_files.contains(&child.path) ||
               (child.is_dir && self.directory_contains_selected_file(child)) {
                return true;
            }
        }
        false
    }

    fn read_file_content(&self, file_path: &Path) -> Result<String> {
        let bytes = fs::read(file_path)
            .map_err(|e| AppError::new_io_error(
                e,
                Some(file_path.to_path_buf()),
                "Failed to read file".to_string(),
            ))?;

        match String::from_utf8(bytes) {
            Ok(content) => {
                // Sanitize content to prevent markdown issues
                let sanitized = content.replace("```", "\\`\\`\\`");
                Ok(sanitized.trim().to_string())
            }
            Err(e) => {
                warn!("File {:?} contains non-UTF8 content, using lossy conversion", file_path);
                let bytes = e.into_bytes();
                let content = String::from_utf8_lossy(&bytes);
                let sanitized = content.replace("```", "\\`\\`\\`");
                Ok(format!(
                    "[WARNING: This file contained non-UTF8 content and was converted with potential data loss]\n\n{}",
                    sanitized.trim()
                ))
            }
        }
    }

    fn get_file_extension(&self, file_path: &Path) -> String {
        file_path.extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("")
            .to_string()
    }

    pub fn atomic_write_markdown(&self, output_path: &Path, content: &str) -> Result<()> {
        let parent_dir = output_path.parent().ok_or_else(|| AppError::AtomicWriteError {
            path: output_path.to_path_buf(),
            details: "Could not get parent directory for temp file.".to_string(),
        })?;

        let mut temp_file = NamedTempFile::new_in(parent_dir)
            .map_err(|e| AppError::new_io_error(
                e,
                None,
                "Failed to create temp file for atomic write.".to_string(),
            ))?;

        temp_file.write_all(content.as_bytes())
            .map_err(|e| AppError::new_io_error(
                e,
                Some(temp_file.path().to_path_buf()),
                "Failed to write to temp file.".to_string(),
            ))?;

        temp_file.persist(output_path)
            .map_err(|e| AppError::AtomicWriteError {
                path: output_path.to_path_buf(),
                details: format!("Failed to persist temp file to target path: {}", e.error),
            })?;

        debug!("Successfully wrote markdown to {:?}", output_path);
        Ok(())
    }

    pub fn update_file_section_in_markdown(
        &self,
        markdown_path: &Path,
        updated_file_path: &Path,
    ) -> Result<()> {
        debug!("Updating markdown section for file: {:?}", updated_file_path);
        
        // Read current markdown content
        let current_content = fs::read_to_string(markdown_path)
            .map_err(|e| AppError::new_io_error(
                e,
                Some(markdown_path.to_path_buf()),
                "Failed to read existing markdown file".to_string(),
            ))?;

        let relative_path = updated_file_path.strip_prefix(&self.directory)
            .map_err(|_| AppError::StripPrefixError {
                prefix: self.directory.clone(),
                path: updated_file_path.to_path_buf(),
            })?;

        let display_path = relative_path.to_string_lossy().replace('\\', "/");
        let file_header = format!("### {}", display_path);
        
        // Find the section to replace
        if let Some(start_index) = current_content.find(&file_header) {
            // Find the end of this section (next ### or end of file)
            let search_start = start_index + file_header.len();
            let end_index = current_content[search_start..]
                .find("\n### ")
                .map(|pos| search_start + pos)
                .unwrap_or(current_content.len());

            // Generate new section for this file
            let new_section = self.generate_file_section(updated_file_path)?;
            
            // Replace the section
            let updated_content = format!(
                "{}{}{}",
                &current_content[..start_index],
                new_section,
                &current_content[end_index..]
            );
            
            self.atomic_write_markdown(markdown_path, &updated_content)?;
            debug!("Successfully updated markdown section for: {}", display_path);
        } else {
            warn!("Could not find section for file {} in markdown", display_path);
            return Err(AppError::MarkdownGeneration(
                format!("Could not find section for file {} in markdown", display_path)
            ));
        }

        Ok(())
    }
} 