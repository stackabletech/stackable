use serde::Serialize;

use crate::engine::NodeRole;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    kind: String,
    api_version: String,
    nodes: Vec<NodeConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            kind: "Cluster".into(),
            api_version: "kind.x-k8s.io/v1alpha4".into(),
            nodes: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NodeConfig {
    role: NodeRole,
}

impl Config {
    pub fn new(node_count: usize, cp_node_count: usize) -> Self {
        let mut cp_node_count = cp_node_count;

        if cp_node_count >= node_count {
            cp_node_count = 1;
        }

        // Create control plane nodes
        let mut control_plane_nodes = Vec::new();

        for _ in 0..cp_node_count {
            control_plane_nodes.push(NodeConfig {
                role: NodeRole::ControlPlane,
            });
        }

        // Create worker nodes
        let mut worker_nodes = Vec::new();

        for _ in 0..node_count - cp_node_count {
            worker_nodes.push(NodeConfig {
                role: NodeRole::Worker,
            })
        }

        Self {
            nodes: [control_plane_nodes, worker_nodes].concat(),
            ..Default::default()
        }
    }
}
