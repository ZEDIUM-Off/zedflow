use std::iter::Peekable;
use std::str::Chars;
use std::sync::Arc;

use serde_json::{Value, json};
use zedflow_agent::types::{
    AgentFuture, AgentTool, AgentToolExecuteFn, AgentToolResult, AgentToolResultContent, Tool,
};
use zedflow_ai::{TextContent, TextContentType};

pub type CalculateResult = AgentToolResult<()>;

pub fn calculate(expression: &str) -> Result<CalculateResult, String> {
    let result = Parser::new(expression).parse()?;
    Ok(AgentToolResult {
        content: vec![text(format!("{expression} = {result}"))],
        details: (),
        terminate: None,
    })
}

pub fn calculate_tool() -> AgentTool<()> {
    let execute: AgentToolExecuteFn<()> = Arc::new(|_tool_call_id, args, _signal, _on_update| {
        let expression = args
            .get("expression")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        Box::pin(async move {
            Ok(
                calculate(&expression).unwrap_or_else(|message| AgentToolResult {
                    content: vec![text(message)],
                    details: (),
                    terminate: Some(true),
                }),
            )
        })
            as AgentFuture<
                '_,
                Result<AgentToolResult<()>, zedflow_agent::types::AgentCallbackError>,
            >
    });

    AgentTool {
        label: "Calculator".into(),
        tool: Tool {
            name: "calculate".into(),
            description: "Evaluate mathematical expressions".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "expression": {
                        "type": "string",
                        "description": "The mathematical expression to evaluate"
                    }
                },
                "required": ["expression"]
            }),
        },
        prepare_arguments: None,
        execute,
        execution_mode: None,
    }
}

fn text(value: impl Into<String>) -> AgentToolResultContent {
    AgentToolResultContent::Text(TextContent {
        content_type: TextContentType::Text,
        text: value.into(),
        text_signature: None,
    })
}

struct Parser<'a> {
    chars: Peekable<Chars<'a>>,
}

impl<'a> Parser<'a> {
    fn new(expression: &'a str) -> Self {
        Self {
            chars: expression.chars().peekable(),
        }
    }

    fn parse(mut self) -> Result<f64, String> {
        let value = self.expression()?;
        self.skip_ws();
        if self.chars.peek().is_some() {
            return Err("unexpected trailing input".into());
        }
        Ok(value)
    }

    fn expression(&mut self) -> Result<f64, String> {
        let mut value = self.term()?;
        loop {
            self.skip_ws();
            match self.chars.peek().copied() {
                Some('+') => {
                    self.chars.next();
                    value += self.term()?;
                }
                Some('-') => {
                    self.chars.next();
                    value -= self.term()?;
                }
                _ => return Ok(value),
            }
        }
    }

    fn term(&mut self) -> Result<f64, String> {
        let mut value = self.factor()?;
        loop {
            self.skip_ws();
            match self.chars.peek().copied() {
                Some('*') => {
                    self.chars.next();
                    value *= self.factor()?;
                }
                Some('/') => {
                    self.chars.next();
                    value /= self.factor()?;
                }
                _ => return Ok(value),
            }
        }
    }

    fn factor(&mut self) -> Result<f64, String> {
        self.skip_ws();
        match self.chars.peek().copied() {
            Some('(') => {
                self.chars.next();
                let value = self.expression()?;
                self.skip_ws();
                if self.chars.next() != Some(')') {
                    return Err("missing closing parenthesis".into());
                }
                Ok(value)
            }
            Some('-') => {
                self.chars.next();
                Ok(-self.factor()?)
            }
            _ => self.number(),
        }
    }

    fn number(&mut self) -> Result<f64, String> {
        self.skip_ws();
        let mut number = String::new();
        while self
            .chars
            .peek()
            .is_some_and(|character| character.is_ascii_digit() || *character == '.')
        {
            number.push(self.chars.next().expect("peeked character exists"));
        }
        if number.is_empty() {
            return Err("expected number".into());
        }
        number.parse::<f64>().map_err(|error| error.to_string())
    }

    fn skip_ws(&mut self) {
        while self
            .chars
            .peek()
            .is_some_and(|character| character.is_whitespace())
        {
            self.chars.next();
        }
    }
}

#[test]
fn calculates_basic_arithmetic() {
    let result = calculate("2 + 2 * 3").unwrap();
    assert_eq!(result.content, vec![text("2 + 2 * 3 = 8")]);
    assert_eq!(result.details, ());
}

#[test]
fn reports_invalid_expressions() {
    assert!(calculate("2 + ").unwrap_err().contains("expected number"));
}

#[test]
fn exposes_calculate_tool_metadata_and_executor() {
    let tool = calculate_tool();
    assert_eq!(tool.label, "Calculator");
    assert_eq!(tool.tool.name, "calculate");
    let execute = tool.execute;
    let result = futures::executor::block_on(execute(
        "call-1",
        json!({ "expression": "(2 + 3) * 4" }),
        None,
        None,
    ));
    assert_eq!(
        result.expect("tool result").content,
        vec![text("(2 + 3) * 4 = 20")]
    );
}
