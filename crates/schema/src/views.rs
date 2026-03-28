use crate::types::{PropType, Schema};

pub fn generate_view_ddl(schema: &Schema) -> Vec<String> {
    schema
        .nodes
        .iter()
        .map(|(name, node_def)| {
            let view_name = format!("v_{}", name.to_lowercase());
            let columns: Vec<String> = node_def
                .properties
                .iter()
                .map(|(prop_name, prop_def)| {
                    let extract = format!("properties->>'$.{prop_name}'");
                    match &prop_def.prop_type {
                        PropType::Int => format!("  CAST({extract} AS INTEGER) AS {prop_name}"),
                        PropType::Float => format!("  CAST({extract} AS DOUBLE) AS {prop_name}"),
                        PropType::Bool => format!("  CAST({extract} AS BOOLEAN) AS {prop_name}"),
                        PropType::Timestamp => {
                            format!("  CAST({extract} AS TIMESTAMP) AS {prop_name}")
                        }
                        PropType::String | PropType::Enum(_) => {
                            format!("  {extract} AS {prop_name}")
                        }
                    }
                })
                .collect();

            let mut parts = vec!["  id".to_string()];
            parts.extend(columns);
            parts.push("  created_at".to_string());
            parts.push("  updated_at".to_string());

            format!(
                "CREATE OR REPLACE VIEW {view_name} AS\nSELECT\n{}\nFROM node WHERE kind = '{name}';",
                parts.join(",\n")
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_session_view() {
        let schema = crate::load_default_schema().unwrap();
        let ddls = generate_view_ddl(&schema);
        let session_ddl = ddls.iter().find(|d| d.contains("v_session")).unwrap();
        assert!(session_ddl.contains("properties->>'$.title'"));
        assert!(session_ddl.contains("FROM node WHERE kind = 'Session'"));
    }
}
