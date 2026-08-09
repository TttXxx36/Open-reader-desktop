use scraper::{Html, Selector};
use serde::Serialize;
use std::time::Instant;

const MAX_XPATH_EXPRESSION_BYTES: usize = 1024;
const MAX_XPATH_HTML_BYTES: usize = 64 * 1024;
const MAX_XPATH_STEPS: usize = 16;
const MAX_XPATH_PREDICATE_BYTES: usize = 256;
const MAX_XPATH_NODE_BUDGET: usize = 4096;
const MAX_XPATH_WORK: usize = 65_536;

#[derive(Debug, Clone, Serialize)]
pub struct XPathAnalysis {
    pub expression: String,
    pub accepted: bool,
    pub syntax: String,
    pub steps: usize,
    pub predicates: usize,
    pub descendant_steps: usize,
    pub terminal_attribute: Option<String>,
    pub html_nodes: usize,
    pub estimated_work: usize,
    pub elapsed_us: u64,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy)]
enum XPathAxis {
    Child,
    Descendant,
}

#[derive(Debug, Clone)]
enum XPathNode {
    Element(String),
    Attribute(String),
}

#[derive(Debug, Clone)]
enum XPathPredicate {
    AttributeEquals { name: String, value: String },
    Position(usize),
}

#[derive(Debug, Clone)]
struct XPathStep {
    axis: XPathAxis,
    node: XPathNode,
    predicate: Option<XPathPredicate>,
}

pub fn analyze(expression: &str, html: &str) -> XPathAnalysis {
    let started = Instant::now();
    let display_expression = truncate_text(expression, MAX_XPATH_EXPRESSION_BYTES);
    let mut analysis = match parse_expression(expression) {
        Err(reason) => XPathAnalysis {
            expression: display_expression,
            accepted: false,
            syntax: "unsupported".to_string(),
            steps: 0,
            predicates: 0,
            descendant_steps: 0,
            terminal_attribute: None,
            html_nodes: 0,
            estimated_work: 0,
            elapsed_us: 0,
            reason: Some(reason),
        },
        Ok(steps) => {
            let predicates = steps.iter().filter(|step| step.predicate.is_some()).count();
            let descendant_steps = steps
                .iter()
                .filter(|step| matches!(step.axis, XPathAxis::Descendant))
                .count();
            let terminal_attribute = steps.last().and_then(|step| match &step.node {
                XPathNode::Attribute(name) => Some(name.clone()),
                XPathNode::Element(_) => None,
            });

            if html.len() > MAX_XPATH_HTML_BYTES {
                XPathAnalysis {
                    expression: display_expression,
                    accepted: false,
                    syntax: "bounded-step-predicate".to_string(),
                    steps: steps.len(),
                    predicates,
                    descendant_steps,
                    terminal_attribute,
                    html_nodes: 0,
                    estimated_work: 0,
                    elapsed_us: 0,
                    reason: Some(format!("离线 HTML 超过 {} 字节上限", MAX_XPATH_HTML_BYTES)),
                }
            } else {
                let html_nodes = count_html_nodes(html);
                let estimated_work = steps.len().saturating_mul(html_nodes).min(MAX_XPATH_WORK);
                let reason = (html_nodes > MAX_XPATH_NODE_BUDGET)
                    .then(|| format!("离线 HTML 节点数超过 {} 项上限", MAX_XPATH_NODE_BUDGET));

                XPathAnalysis {
                    expression: display_expression,
                    accepted: reason.is_none(),
                    syntax: "bounded-step-predicate".to_string(),
                    steps: steps.len(),
                    predicates,
                    descendant_steps,
                    terminal_attribute,
                    html_nodes,
                    estimated_work,
                    elapsed_us: 0,
                    reason,
                }
            }
        }
    };
    analysis.elapsed_us = started
        .elapsed()
        .as_micros()
        .max(1)
        .min(u128::from(u64::MAX)) as u64;
    analysis
}

fn count_html_nodes(html: &str) -> usize {
    let document = Html::parse_document(html);
    let selector = Selector::parse("*").expect("universal selector is valid");
    document.select(&selector).count()
}

fn parse_expression(expression: &str) -> Result<Vec<XPathStep>, String> {
    let expression = expression.trim();
    if expression.is_empty() {
        return Err("XPath 表达式不能为空".to_string());
    }
    if expression.len() > MAX_XPATH_EXPRESSION_BYTES {
        return Err(format!(
            "XPath 表达式不能超过 {} 字节",
            MAX_XPATH_EXPRESSION_BYTES
        ));
    }
    if !expression.starts_with('/') {
        return Err("XPath 只支持以 / 或 // 开头的路径".to_string());
    }
    if expression.contains('|')
        || expression.contains("::")
        || expression.contains("..")
        || expression.contains('(')
        || expression.contains(')')
    {
        return Err("XPath 不支持联合、轴、父节点或函数表达式".to_string());
    }

    let mut remaining = expression;
    let mut steps = Vec::new();
    while !remaining.is_empty() {
        let (axis, separator_len) = if remaining.starts_with("//") {
            (XPathAxis::Descendant, 2)
        } else if remaining.starts_with('/') {
            (XPathAxis::Child, 1)
        } else {
            return Err("XPath 路径分隔符无效".to_string());
        };
        remaining = &remaining[separator_len..];
        if remaining.is_empty() {
            return Err("XPath 路径不能以分隔符结尾".to_string());
        }

        let end = remaining.find('/').unwrap_or(remaining.len());
        let raw_step = &remaining[..end];
        if raw_step.is_empty() {
            return Err("XPath 路径包含空步骤".to_string());
        }
        let step = parse_step(axis, raw_step)?;
        if matches!(&step.node, XPathNode::Attribute(_)) && end < remaining.len() {
            return Err("XPath 属性读取必须是最后一步".to_string());
        }
        steps.push(step);
        if steps.len() > MAX_XPATH_STEPS {
            return Err(format!("XPath 步骤不能超过 {} 项", MAX_XPATH_STEPS));
        }
        remaining = &remaining[end..];
    }

    Ok(steps)
}

fn parse_step(axis: XPathAxis, raw_step: &str) -> Result<XPathStep, String> {
    let (node_text, predicate) = if let Some(open) = raw_step.find('[') {
        if !raw_step.ends_with(']') || raw_step[open + 1..raw_step.len() - 1].contains('[') {
            return Err("XPath 谓词括号不完整".to_string());
        }
        let predicate_text = &raw_step[open + 1..raw_step.len() - 1];
        if predicate_text.len() > MAX_XPATH_PREDICATE_BYTES {
            return Err(format!(
                "XPath 谓词不能超过 {} 字节",
                MAX_XPATH_PREDICATE_BYTES
            ));
        }
        (&raw_step[..open], Some(parse_predicate(predicate_text)?))
    } else {
        (raw_step, None)
    };

    if node_text.starts_with('@') {
        if predicate.is_some() {
            return Err("XPath 属性步骤不支持谓词".to_string());
        }
        return Ok(XPathStep {
            axis,
            node: XPathNode::Attribute(parse_name(&node_text[1..])?),
            predicate: None,
        });
    }

    let node = if node_text == "*" {
        XPathNode::Element("*".to_string())
    } else {
        XPathNode::Element(parse_name(node_text)?)
    };
    Ok(XPathStep {
        axis,
        node,
        predicate,
    })
}

fn parse_predicate(predicate: &str) -> Result<XPathPredicate, String> {
    let predicate = predicate.trim();
    if predicate.is_empty() {
        return Err("XPath 谓词不能为空".to_string());
    }
    if let Ok(position) = predicate.parse::<usize>() {
        if position == 0 {
            return Err("XPath 位置谓词必须从 1 开始".to_string());
        }
        return Ok(XPathPredicate::Position(position));
    }

    let predicate = predicate
        .strip_prefix('@')
        .ok_or_else(|| "XPath 仅支持属性等值谓词".to_string())?;
    let (name, expected) = predicate
        .split_once('=')
        .ok_or_else(|| "XPath 属性谓词必须使用等号".to_string())?;
    let name = parse_name(name.trim())?;
    let expected = expected.trim();
    let quote = expected
        .chars()
        .next()
        .filter(|character| *character == '\'' || *character == '"')
        .ok_or_else(|| "XPath 属性谓词必须使用引号".to_string())?;
    if expected.len() < 2 || !expected.ends_with(quote) {
        return Err("XPath 属性谓词引号不完整".to_string());
    }
    let value = expected[1..expected.len() - 1].to_string();
    if value.contains(quote) {
        return Err("XPath 仅支持单一属性等值谓词".to_string());
    }
    Ok(XPathPredicate::AttributeEquals { name, value })
}

fn parse_name(value: &str) -> Result<String, String> {
    let value = value.trim();
    let mut characters = value.chars();
    let Some(first) = characters.next() else {
        return Err("XPath 节点名不能为空".to_string());
    };
    if !(first.is_ascii_alphabetic() || first == '_')
        || !characters
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
    {
        return Err("XPath 节点名只支持 ASCII 字母、数字、下划线和短横线".to_string());
    }
    Ok(value.to_string())
}

fn truncate_text(value: &str, max_bytes: usize) -> String {
    let mut output = String::new();
    for character in value.trim().chars() {
        if output.len() + character.len_utf8() > max_bytes {
            output.push('…');
            break;
        }
        output.push(character);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bounded_xpath_against_synthetic_html() {
        let analysis = analyze(
            "//article[@class='book']//a/@href",
            r#"<main><article class="book"><a href="/book/1">一本书</a></article></main>"#,
        );
        assert!(analysis.accepted, "{analysis:?}");
        assert_eq!(analysis.steps, 3);
        assert_eq!(analysis.predicates, 1);
        assert_eq!(analysis.descendant_steps, 2);
        assert_eq!(analysis.terminal_attribute.as_deref(), Some("href"));
        assert!(analysis.html_nodes >= 3);
        assert!(analysis.estimated_work > 0);
    }

    #[test]
    fn rejects_xpath_functions_axes_and_union() {
        for expression in [
            "//article/text()",
            "//article/../a",
            "//article | //a",
            "//article::a",
        ] {
            let analysis = analyze(expression, "<article><a>demo</a></article>");
            assert!(!analysis.accepted, "{expression}: {analysis:?}");
            assert!(analysis.reason.is_some());
        }
    }

    #[test]
    fn covers_common_xpath_fixture_matrix() {
        let html = r#"
            <html><body><main>
                <article class="book"><a href="/book/1">一本书</a></article>
                <article class="book"><a href="/book/2">第二本</a></article>
            </main></body></html>
        "#;
        let fixtures = [
            ("//article", true),
            ("/html/body/main/article", true),
            ("//article[@class='book']", true),
            ("//article[1]", true),
            ("//*[@id=\"content\"]", true),
            ("//a/@href", true),
            ("//article[contains(@class,'book')]", false),
            ("//article/@href/text()", false),
            ("//article | //a", false),
            ("//article::a", false),
            ("//article/../a", false),
            ("//article[@class!='book']", false),
        ];

        let mut accepted = 0;
        for (expression, expected) in fixtures {
            let analysis = analyze(expression, html);
            assert_eq!(analysis.accepted, expected, "{expression}: {analysis:?}");
            if expected {
                accepted += 1;
                assert!(analysis.steps <= MAX_XPATH_STEPS);
                assert!(analysis.estimated_work <= MAX_XPATH_WORK);
            }
        }
        assert_eq!(accepted, 6);

        let dense_html = format!("<main>{}</main>", "<article></article>".repeat(128));
        let dense = analyze("//article", &dense_html);
        assert!(dense.accepted);
        assert!(dense.html_nodes >= 129);
        assert!(dense.estimated_work > 0);
    }

    #[test]
    fn records_authorized_fixture_timing_distribution() {
        let html = r#"
            <html><body><main>
                <article class="book"><a href="/book/1">一本书</a></article>
                <article class="book"><a href="/book/2">第二本</a></article>
            </main></body></html>
        "#;
        let expressions = [
            "//article",
            "//article[@class='book']",
            "//article[1]",
            "//*[@class='book']//a/@href",
            "/html/body/main/article",
            "//article[contains(@class,'book')]",
            "//article | //a",
            "//article/../a",
        ];

        let mut elapsed = expressions
            .iter()
            .map(|expression| analyze(expression, html).elapsed_us)
            .collect::<Vec<_>>();
        assert!(elapsed.iter().all(|duration| *duration >= 1));
        elapsed.sort_unstable();
        let p50 = elapsed[elapsed.len() / 2];
        let p95 = elapsed[(elapsed.len() * 95).div_ceil(100).saturating_sub(1)];
        assert!(p95 >= p50);
        assert!(
            p95 < 2_000_000,
            "synthetic XPath fixture parse p95 exceeded 2 seconds: {elapsed:?}"
        );
    }

    #[test]
    fn rejects_malformed_xpath_regressions() {
        for expression in [
            "",
            "article",
            "/",
            "//",
            "//article/",
            "//article[]",
            "//article[0]",
            "//article[@class]",
            "//article[@class='book' and @id='1']",
            "//article[foo]",
            "//article[@class='book']/@href/text()",
        ] {
            let analysis = analyze(expression, "<main><article class="book"><a>demo</a></article></main>");
            assert!(!analysis.accepted, "{expression}: {analysis:?}");
            assert!(analysis.reason.is_some(), "{expression}: {analysis:?}");
        }
    }

    #[test]
    fn enforces_synthetic_html_node_budget() {
        let html = format!("<main>{}</main>", "<a></a>".repeat(MAX_XPATH_NODE_BUDGET));
        let analysis = analyze("//a", &html);
        assert!(!analysis.accepted, "{analysis:?}");
        assert_eq!(analysis.html_nodes, MAX_XPATH_NODE_BUDGET + 1);
        assert!(analysis
            .reason
            .as_deref()
            .is_some_and(|reason| reason.contains("节点数超过")));
    }

    #[test]
    fn enforces_xpath_expression_and_html_budgets() {
        let expression = format!("//{}", "a".repeat(MAX_XPATH_EXPRESSION_BYTES));
        let analysis = analyze(&expression, "<a>demo</a>");
        assert!(!analysis.accepted);
        assert!(analysis
            .reason
            .as_deref()
            .is_some_and(|reason| reason.contains("不能超过")));

        let html = format!("<div>{}</div>", "x".repeat(MAX_XPATH_HTML_BYTES));
        let analysis = analyze("//div", &html);
        assert!(!analysis.accepted);
        assert!(analysis.reason.is_some());
    }

    #[test]
    fn rejects_unsupported_predicates() {
        for expression in [
            "//article[@class]",
            "//article[@class!='book']",
            "//article[foo]",
        ] {
            let analysis = analyze(expression, "<article>demo</article>");
            assert!(!analysis.accepted, "{expression}: {analysis:?}");
        }
    }
}
