#[derive(Debug, Clone, PartialEq)]
pub struct EdgeStep {
    pub kind: Option<String>, // None = wildcard (*)
    pub reverse: bool,
}

pub fn parse_edge_path(path: &str) -> Vec<EdgeStep> {
    path.split('/')
        .map(|segment| {
            let (reverse, name) = if let Some(stripped) = segment.strip_prefix('~') {
                (true, stripped)
            } else {
                (false, segment)
            };

            let kind = if name == "*" { None } else { Some(name.to_string()) };

            EdgeStep { kind, reverse }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_hop() {
        let steps = parse_edge_path("HAS_TOPIC");
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].kind.as_deref(), Some("HAS_TOPIC"));
        assert!(!steps[0].reverse);
    }

    #[test]
    fn reverse_hop() {
        let steps = parse_edge_path("~HAS_TOPIC");
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].kind.as_deref(), Some("HAS_TOPIC"));
        assert!(steps[0].reverse);
    }

    #[test]
    fn multi_hop() {
        let steps = parse_edge_path("HAS_TOPIC/~HAS_TOPIC");
        assert_eq!(steps.len(), 2);
        assert!(!steps[0].reverse);
        assert!(steps[1].reverse);
    }

    #[test]
    fn wildcard() {
        let steps = parse_edge_path("*");
        assert_eq!(steps.len(), 1);
        assert!(steps[0].kind.is_none());
        assert!(!steps[0].reverse);
    }

    #[test]
    fn mixed_path() {
        let steps = parse_edge_path("*/~HAS_TOPIC/CONTINUES");
        assert_eq!(steps.len(), 3);
        assert!(steps[0].kind.is_none());
        assert!(steps[1].reverse);
        assert_eq!(steps[2].kind.as_deref(), Some("CONTINUES"));
    }
}
