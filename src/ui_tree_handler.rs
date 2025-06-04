use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use egui::{Id, Ui, CollapsingHeader, Checkbox};
use log::debug;

use crate::file_handler::FileNode;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectionState {
    Unselected,
    Selected,
    PartiallySelected,
}

#[derive(Clone, Debug)]
pub struct UITreeNode {
    pub id: Id,
    pub file_node_path: PathBuf,
    pub display_name: String,
    pub is_dir: bool,
    pub selected_state: SelectionState,
    pub expanded: bool,
    pub children_indices: Vec<usize>,
    pub parent_index: Option<usize>,
}

pub struct UITreeHandler {
    pub tree_nodes: Vec<UITreeNode>,
    pub selected_files: HashSet<PathBuf>,
    path_to_index: HashMap<PathBuf, usize>,
}

impl UITreeHandler {
    pub fn new() -> Self {
        Self {
            tree_nodes: Vec::new(),
            selected_files: HashSet::new(),
            path_to_index: HashMap::new(),
        }
    }

    pub fn build_from_file_node(&mut self, root_node: &FileNode) {
        self.tree_nodes.clear();
        self.path_to_index.clear();
        
        debug!("Building UI tree from root node: {:?}", root_node.name);
        debug!("Root node has {} children", root_node.children.len());
        
        self.build_tree_recursive(root_node, None);
        self.update_all_selection_states();
        
        debug!("Built UI tree with {} nodes total", self.tree_nodes.len());
        
        // Log all nodes for debugging
        for (i, node) in self.tree_nodes.iter().enumerate() {
            debug!("Node {}: {} (is_dir: {}, children: {})", 
                   i, node.display_name, node.is_dir, node.children_indices.len());
        }
    }

    fn build_tree_recursive(&mut self, node: &FileNode, parent_index: Option<usize>) -> usize {
        let node_index = self.tree_nodes.len();
        
        // Generate stable ID from path
        let id = Id::new(&node.path);
        
        let ui_node = UITreeNode {
            id,
            file_node_path: node.path.clone(),
            display_name: node.name.clone(),
            is_dir: node.is_dir,
            selected_state: if self.selected_files.contains(&node.path) {
                SelectionState::Selected
            } else {
                SelectionState::Unselected
            },
            expanded: false, // Default to collapsed
            children_indices: Vec::new(),
            parent_index,
        };
        
        self.tree_nodes.push(ui_node);
        self.path_to_index.insert(node.path.clone(), node_index);
        
        // Process children
        let mut children_indices = Vec::new();
        for child in &node.children {
            let child_index = self.build_tree_recursive(child, Some(node_index));
            children_indices.push(child_index);
        }
        
        // Update children indices
        self.tree_nodes[node_index].children_indices = children_indices;
        
        node_index
    }

    pub fn render_tree(&mut self, ui: &mut Ui) -> bool {
        let mut selection_changed = false;
        
        if !self.tree_nodes.is_empty() {
            selection_changed = self.render_node_recursive(ui, 0);
        }
        
        if selection_changed {
            self.update_selected_files();
            self.update_all_selection_states();
        }
        
        selection_changed
    }

    fn render_node_recursive(&mut self, ui: &mut Ui, node_index: usize) -> bool {
        let mut selection_changed = false;
        
        // Clone the node data to avoid borrowing issues
        let node = self.tree_nodes[node_index].clone();
        
        if node.is_dir {
            // Render directory as collapsing header with checkbox
            ui.horizontal(|ui| {
                // Checkbox for directory
                let mut selected = node.selected_state == SelectionState::Selected;
                let checkbox_response = ui.add(Checkbox::new(&mut selected, ""));
                
                if checkbox_response.clicked() {
                    self.toggle_node_selection(node_index);
                    selection_changed = true;
                }
                
                // Add some visual indication for partially selected directories
                // let header_icon = match node.selected_state {
                //     SelectionState::Selected => "📁",
                //     SelectionState::PartiallySelected => "📂",
                //     SelectionState::Unselected => "📁",
                // };
                
                // Collapsing header for directory with better styling
                let header_response = CollapsingHeader::new(format!(" {}", node.display_name))
                    .id_source(node.id)
                    .default_open(node.expanded)
                    .show(ui, |ui| {
                        // Add some padding for nested content
                        ui.add_space(2.0);
                        
                        // Render children with better indentation
                        for &child_index in &node.children_indices {
                            ui.horizontal(|ui| {
                                ui.add_space(10.0); // Indent children
                                ui.vertical(|ui| {
                                    if self.render_node_recursive(ui, child_index) {
                                        selection_changed = true;
                                    }
                                });
                            });
                        }
                    });
                
                // Update expanded state
                self.tree_nodes[node_index].expanded = header_response.openness > 0.5;
            });
        } else {
            // Render file as checkbox with label and appropriate icon
            ui.horizontal(|ui| {
                let mut selected = node.selected_state == SelectionState::Selected;
                let checkbox_response = ui.add(Checkbox::new(&mut selected, ""));
                
                if checkbox_response.clicked() {
                    self.toggle_node_selection(node_index);
                    selection_changed = true;
                }
                
                // Style the file name based on selection
                if selected {
                    ui.colored_label(egui::Color32::from_rgb(0, 120, 0), format!("{}", node.display_name));
                } else {
                    ui.label(format!("{}", node.display_name));
                }
            });
        }
        
        selection_changed
    }

    fn toggle_node_selection(&mut self, node_index: usize) {
        let current_state = &self.tree_nodes[node_index].selected_state;
        let new_state = match current_state {
            SelectionState::Selected => SelectionState::Unselected,
            SelectionState::Unselected | SelectionState::PartiallySelected => SelectionState::Selected,
        };
        
        self.tree_nodes[node_index].selected_state = new_state.clone();
        
        // If this is a directory, propagate to children
        if self.tree_nodes[node_index].is_dir {
            self.propagate_selection_to_children(node_index, &new_state);
        }
        
        // Update parent states
        if let Some(parent_index) = self.tree_nodes[node_index].parent_index {
            self.update_parent_selection_state(parent_index);
        }
    }

    fn propagate_selection_to_children(&mut self, node_index: usize, state: &SelectionState) {
        let children_indices = self.tree_nodes[node_index].children_indices.clone();
        
        for child_index in children_indices {
            self.tree_nodes[child_index].selected_state = state.clone();
            
            // Recursively propagate to grandchildren if this child is a directory
            if self.tree_nodes[child_index].is_dir {
                self.propagate_selection_to_children(child_index, state);
            }
        }
    }

    fn update_parent_selection_state(&mut self, parent_index: usize) {
        let children_indices = self.tree_nodes[parent_index].children_indices.clone();
        
        if children_indices.is_empty() {
            return;
        }
        
        let mut all_selected = true;
        let mut all_unselected = true;
        
        for &child_index in &children_indices {
            match &self.tree_nodes[child_index].selected_state {
                SelectionState::Selected => all_unselected = false,
                SelectionState::Unselected => all_selected = false,
                SelectionState::PartiallySelected => {
                    all_selected = false;
                    all_unselected = false;
                }
            }
        }
        
        let new_state = if all_selected {
            SelectionState::Selected
        } else if all_unselected {
            SelectionState::Unselected
        } else {
            SelectionState::PartiallySelected
        };
        
        self.tree_nodes[parent_index].selected_state = new_state;
        
        // Recursively update grandparent
        if let Some(grandparent_index) = self.tree_nodes[parent_index].parent_index {
            self.update_parent_selection_state(grandparent_index);
        }
    }

    fn update_all_selection_states(&mut self) {
        // Start from leaf nodes and work upward
        let mut leaf_indices = Vec::new();
        for (i, node) in self.tree_nodes.iter().enumerate() {
            if node.children_indices.is_empty() {
                leaf_indices.push(i);
            }
        }
        
        // Update parent states for all leaf nodes
        for &leaf_index in &leaf_indices {
            if let Some(parent_index) = self.tree_nodes[leaf_index].parent_index {
                self.update_parent_selection_state(parent_index);
            }
        }
    }

    fn update_selected_files(&mut self) {
        self.selected_files.clear();
        
        for node in &self.tree_nodes {
            if node.selected_state == SelectionState::Selected && !node.is_dir {
                self.selected_files.insert(node.file_node_path.clone());
            }
        }
        
        debug!("Updated selected files: {} files selected", self.selected_files.len());
    }

    pub fn get_selected_files(&self) -> Vec<PathBuf> {
        self.selected_files.iter().cloned().collect()
    }

    #[allow(dead_code)]
    pub fn set_selected_files(&mut self, files: HashSet<PathBuf>) {
        self.selected_files = files;
        
        // Update UI state to match
        for node in &mut self.tree_nodes {
            if !node.is_dir {
                node.selected_state = if self.selected_files.contains(&node.file_node_path) {
                    SelectionState::Selected
                } else {
                    SelectionState::Unselected
                };
            }
        }
        
        self.update_all_selection_states();
    }

    pub fn has_selection(&self) -> bool {
        !self.selected_files.is_empty()
    }

    #[allow(dead_code)]
    pub fn clear_selection(&mut self) {
        self.selected_files.clear();
        
        for node in &mut self.tree_nodes {
            node.selected_state = SelectionState::Unselected;
        }
    }
} 