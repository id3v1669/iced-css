use super::SourceNode;
use std::collections::{BTreeMap, BTreeSet};

/// Read-only candidate preflight. This does not move native state or publish a host.
pub fn validate_tree(nodes: &[SourceNode]) -> Result<(), String> {
    let mut index = BTreeMap::new();
    let mut css_ids = BTreeSet::new();
    let mut root = None;
    for node in nodes {
        if index.insert(node.instance, node).is_some() {
            return Err("duplicate source instance".into());
        }
        if node.parent.is_none() && root.replace(node.instance).is_some() {
            return Err("multiple source roots".into());
        }
        if let Some(id) = node.metadata.css_id()
            && !css_ids.insert(id)
        {
            return Err(format!("duplicate CSS ID: {id}"));
        }
    }
    let root = root.ok_or("missing source root")?;
    for node in nodes {
        let mut children = BTreeSet::new();
        for child in &node.children {
            if !children.insert(child) {
                return Err("duplicate source child".into());
            }
            let child_node = index.get(child).ok_or("unknown source child")?;
            if child_node.parent != Some(node.instance) {
                return Err("nonreciprocal source parent".into());
            }
        }
        if let Some(parent) = node.parent {
            let parent = index.get(&parent).ok_or("unknown source parent")?;
            if !parent.children.contains(&node.instance) {
                return Err("parent omits source child".into());
            }
        }
    }
    let mut visited = BTreeSet::new();
    let mut stack = vec![root];
    while let Some(id) = stack.pop() {
        if !visited.insert(id) {
            return Err("cyclic or shared source membership".into());
        }
        stack.extend(index[&id].children.iter().copied());
    }
    if visited.len() != nodes.len() {
        return Err("unreachable source component".into());
    }
    Ok(())
}
