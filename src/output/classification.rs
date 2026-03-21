use colored::Colorize;

use super::common::{print_formatted, truncate};
use crate::types::{Classification, OutputFormat};

#[expect(clippy::print_stdout)]
pub fn print_classification(classification: &Classification, format: OutputFormat) {
    print_formatted(classification, format, |classification| {
        println!(
            "{} {}\n{}\n",
            "Classification".bold(),
            classification.name.bold(),
            classification.description,
        );
        if !classification.products.is_empty() {
            println!("{}:", "Products".bold());
            for p in &classification.products {
                println!("  {} - {}", p.name, truncate(&p.description, 60));
            }
        }
    });
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use crate::types::{Classification, ClassificationProduct};

    #[test]
    fn print_classification_json() {
        let classification = Classification {
            id: 1,
            name: "Software".into(),
            description: "Software products".into(),
            sort_key: 0,
            products: vec![ClassificationProduct {
                id: 10,
                name: "Widget".into(),
                description: "Widget product".into(),
            }],
        };
        let json = serde_json::to_string(&classification).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["name"], "Software");
        assert_eq!(parsed["products"][0]["name"], "Widget");
    }

    #[test]
    fn print_classification_json_empty_products() {
        let classification = Classification {
            id: 2,
            name: "Empty".into(),
            description: "No products".into(),
            sort_key: 0,
            products: vec![],
        };
        let json = serde_json::to_string(&classification).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["products"].as_array().unwrap().len(), 0);
    }
}
