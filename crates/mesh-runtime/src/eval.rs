//! Evaluating expressions (§9.7): left to right, each operand before its
//! operator, and the first failure is the one reported.

use crate::diagnostic::{Location, RuntimeCode, RuntimeDiagnostic};
use crate::types::Statics;
use crate::value::Value;
use mesh_template::{BinaryOperator, Expression, Literal, Span, UnaryOperator};
use std::collections::BTreeMap;
use std::rc::Rc;

/// Where an expression is evaluated: in the template of `component`,
/// with its scope's values, and, in a handler's arguments, the payload.
pub(crate) struct Scope<'a, 'm> {
    pub component: &'a str,
    pub values: &'a BTreeMap<String, Value>,
    pub payload: Option<&'a Value>,
    pub statics: Statics<'m>,
}

impl Scope<'_, '_> {
    pub(crate) fn error(
        &self,
        code: RuntimeCode,
        message: impl Into<String>,
        span: Span,
    ) -> RuntimeDiagnostic {
        RuntimeDiagnostic::new(
            code,
            message,
            Location::Source {
                component: self.component.to_string(),
                span,
            },
        )
    }

    fn operand(&self, value: &Value, needs: &str, span: Span) -> RuntimeDiagnostic {
        self.error(
            RuntimeCode::OPERAND_MISMATCH,
            format!("this operand is {}, but must be {needs}", value.kind()),
            span,
        )
    }

    fn boolean(&self, expression: &Expression) -> Result<bool, RuntimeDiagnostic> {
        match self.eval(expression)? {
            Value::Boolean(value) => Ok(value),
            other => Err(self.operand(&other, "a boolean", expression.span())),
        }
    }

    /// Evaluates `expression`.
    pub(crate) fn eval(&self, expression: &Expression) -> Result<Value, RuntimeDiagnostic> {
        Ok(match expression {
            Expression::Literal { value, .. } => match value {
                Literal::String(text) => Value::String(Rc::from(text.as_str())),
                Literal::Number(number) => Value::Number(*number),
                Literal::Boolean(value) => Value::Boolean(*value),
                Literal::Null => Value::Null,
            },
            Expression::Scope { name, .. } => {
                self.values.get(name).cloned().unwrap_or(Value::Absent)
            }
            Expression::Member {
                object,
                field,
                span,
            } => match self.eval(object)? {
                Value::Record(record) => match record.get(field) {
                    Some(value) => value.clone(),
                    // On a record type, an absent field reads as absent;
                    // on `any`, the field must be present (§9.7.6).
                    None if self.statics.is_any(&self.statics.of(object)) => {
                        return Err(self.error(
                            RuntimeCode::MISSING_MEMBER,
                            format!("this value has no field `{field}`"),
                            *span,
                        ))
                    }
                    None => Value::Absent,
                },
                other => {
                    return Err(self.error(
                        RuntimeCode::NOT_A_RECORD,
                        format!(
                            "this is {}, not a record, so it has no field `{field}`",
                            other.kind()
                        ),
                        object.span(),
                    ))
                }
            },
            Expression::Unary {
                operator, operand, ..
            } => match operator {
                UnaryOperator::Not => Value::Boolean(!self.boolean(operand)?),
                UnaryOperator::Negate => match self.eval(operand)? {
                    Value::Number(number) => Value::Number(-number),
                    other => return Err(self.operand(&other, "a number", operand.span())),
                },
            },
            Expression::Binary {
                operator,
                left,
                right,
                ..
            } => self.binary(*operator, left, right)?,
            Expression::Conditional {
                condition,
                consequent,
                alternate,
                ..
            } => {
                if self.boolean(condition)? {
                    self.eval(consequent)?
                } else {
                    self.eval(alternate)?
                }
            }
            Expression::List { elements, .. } => Value::List(
                elements
                    .iter()
                    .map(|element| self.eval(element))
                    .collect::<Result<Vec<_>, _>>()?
                    .into(),
            ),
            Expression::Record { fields, .. } => {
                let mut record = BTreeMap::new();
                for field in fields {
                    let value = self.eval(&field.value)?;
                    // A field whose value is absent is no field (§9.7.1).
                    if !matches!(value, Value::Absent) {
                        record.insert(field.name.clone(), value);
                    }
                }
                Value::Record(Rc::new(record))
            }
            Expression::Event { .. } => self.payload.cloned().unwrap_or(Value::Absent),
        })
    }

    fn binary(
        &self,
        operator: BinaryOperator,
        left: &Expression,
        right: &Expression,
    ) -> Result<Value, RuntimeDiagnostic> {
        use BinaryOperator as B;
        match operator {
            B::And => return Ok(Value::Boolean(self.boolean(left)? && self.boolean(right)?)),
            B::Or => return Ok(Value::Boolean(self.boolean(left)? || self.boolean(right)?)),
            _ => {}
        }
        let (a, b) = (self.eval(left)?, self.eval(right)?);
        match operator {
            B::Equal => return Ok(Value::Boolean(a.equals(&b))),
            B::NotEqual => return Ok(Value::Boolean(!a.equals(&b))),
            _ => {}
        }
        let number = |value: &Value, expression: &Expression| match value {
            Value::Number(number) => Ok(*number),
            other => Err(self.operand(other, "a number", expression.span())),
        };
        let (x, y) = (number(&a, left)?, number(&b, right)?);
        Ok(match operator {
            B::Add => Value::Number(x + y),
            B::Subtract => Value::Number(x - y),
            B::Multiply => Value::Number(x * y),
            B::Divide => Value::Number(x / y),
            // Rust's `%` is the truncated remainder, computed exactly (§9.7.3).
            B::Remainder => Value::Number(x % y),
            B::Less => Value::Boolean(x < y),
            B::LessEqual => Value::Boolean(x <= y),
            B::Greater => Value::Boolean(x > y),
            B::GreaterEqual => Value::Boolean(x >= y),
            B::And | B::Or | B::Equal | B::NotEqual => unreachable!("handled above"),
        })
    }
}
