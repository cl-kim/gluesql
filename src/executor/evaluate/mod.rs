mod error;
mod evaluated;

use std::fmt::Debug;

use sqlparser::ast::{BinaryOperator, Expr, Value as AstValue};

use crate::executor::{select, FilterContext};
use crate::result::Result;
use crate::storage::Store;

pub use error::EvaluateError;
pub use evaluated::Evaluated;

pub fn evaluate<'a, T: 'static + Debug>(
    storage: &'a dyn Store<T>,
    filter_context: &'a FilterContext<'a>,
    expr: &'a Expr,
) -> Result<Evaluated<'a>> {
    let eval = |expr| evaluate(storage, filter_context, expr);

    match expr {
        Expr::Value(value) => match value {
            v @ AstValue::Number(_) | v @ AstValue::Boolean(_) => Ok(Evaluated::LiteralRef(v)),
            _ => Err(EvaluateError::Unimplemented.into()),
        },
        Expr::Identifier(ident) => match ident.quote_style {
            Some(_) => Ok(Evaluated::StringRef(&ident.value)),
            None => filter_context
                .get_value(&ident.value)
                .map(Evaluated::ValueRef),
        },
        Expr::Nested(expr) => eval(&expr),
        Expr::CompoundIdentifier(idents) => {
            if idents.len() != 2 {
                return Err(EvaluateError::UnsupportedCompoundIdentifier(expr.to_string()).into());
            }

            let table_alias = &idents[0].value;
            let column = &idents[1].value;

            filter_context
                .get_alias_value(table_alias, column)
                .map(Evaluated::ValueRef)
        }
        Expr::Subquery(query) => select(storage, &query, Some(filter_context))?
            .map(|row| row?.take_first_value())
            .map(|value| value.map(Evaluated::Value))
            .next()
            .ok_or(EvaluateError::NestedSelectRowNotFound)?,
        Expr::BinaryOp { op, left, right } => {
            let l = eval(left)?;
            let r = eval(right)?;

            match op {
                BinaryOperator::Plus => l.add(&r),
                BinaryOperator::Minus => l.subtract(&r),
                BinaryOperator::Multiply => l.multiply(&r),
                BinaryOperator::Divide => l.divide(&r),
                _ => Err(EvaluateError::Unimplemented.into()),
            }
        }
        _ => Err(EvaluateError::Unimplemented.into()),
    }
}
