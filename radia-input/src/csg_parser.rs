use nom::{
    IResult,
    bytes::complete::take_while1,
    character::complete::{char, multispace0},
};
use radia_core::csg::{
    CSGInstructionValidationError, FlatCSG, Instruction, validate_csg_instructions,
};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Primitive(usize),
    Union,        // +
    Intersection, // *
    Difference,   // -
    Complement,   // !
    LParen,       // (
    RParen,       // )
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Assoc {
    Left,
    Right,
}

#[derive(thiserror::Error, Debug)]
pub enum CSGParseError {
    #[error("Lex error: {0}")]
    LexError(String),
    #[error("Shunting yard error: {0}")]
    ShuntingYardError(String),
    #[error("Validation error: {0}")]
    CSGInstructionValidationError(#[from] CSGInstructionValidationError),
}

fn precedence(token: &Token) -> Option<(u8, Assoc)> {
    match token {
        Token::Union => Some((1, Assoc::Left)),
        Token::Difference => Some((2, Assoc::Left)),
        Token::Intersection => Some((3, Assoc::Left)),
        Token::Complement => Some((4, Assoc::Right)),
        _ => None,
    }
}

fn parse_identifier(input: &str) -> IResult<&str, &str> {
    take_while1(|c: char| c.is_alphanumeric() || c == '_')(input)
}

fn lex_token<'a>(input: &'a str, prim_map: &HashMap<String, usize>) -> IResult<&'a str, Token> {
    let (input, _) = multispace0(input)?;

    if let Ok((rest, _)) = char::<&str, nom::error::Error<&str>>('(')(input) {
        return Ok((rest, Token::LParen));
    }
    if let Ok((rest, _)) = char::<&str, nom::error::Error<&str>>(')')(input) {
        return Ok((rest, Token::RParen));
    }
    if let Ok((rest, _)) = char::<&str, nom::error::Error<&str>>('+')(input) {
        return Ok((rest, Token::Union));
    }
    if let Ok((rest, _)) = char::<&str, nom::error::Error<&str>>('*')(input) {
        return Ok((rest, Token::Intersection));
    }
    if let Ok((rest, _)) = char::<&str, nom::error::Error<&str>>('-')(input) {
        return Ok((rest, Token::Difference));
    }
    if let Ok((rest, _)) = char::<&str, nom::error::Error<&str>>('!')(input) {
        return Ok((rest, Token::Complement));
    }

    let (rest, name) = parse_identifier(input)?;
    let idx = prim_map.get(name).copied().ok_or_else(|| {
        nom::Err::Failure(nom::error::Error::new(input, nom::error::ErrorKind::Tag))
    })?;
    Ok((rest, Token::Primitive(idx)))
}

fn tokenize(
    mut input: &str,
    prim_map: &HashMap<String, usize>,
) -> Result<Vec<Token>, CSGParseError> {
    let mut tokens = Vec::new();

    loop {
        let (rest, _) = multispace0::<&str, nom::error::Error<&str>>(input).unwrap();
        if rest.is_empty() {
            break;
        }
        match lex_token(rest, prim_map) {
            Ok((rest, tok)) => {
                tokens.push(tok);
                input = rest;
            }
            Err(e) => return Err(CSGParseError::LexError(format!("{:?}", e))),
        }
    }

    Ok(tokens)
}

fn shunting_yard(tokens: Vec<Token>) -> Result<Vec<Instruction>, CSGParseError> {
    let mut output: Vec<Instruction> = Vec::new();
    let mut op_stack: Vec<Token> = Vec::new();

    for token in tokens {
        match &token {
            Token::Primitive(idx) => {
                output.push(Instruction::PushPrimitive(*idx));
            }
            Token::Complement => {
                op_stack.push(token);
            }
            Token::Union | Token::Intersection | Token::Difference => {
                let (prec, assoc) = precedence(&token).unwrap();
                while let Some(top) = op_stack.last() {
                    if *top == Token::LParen {
                        break;
                    }
                    let top_prec = precedence(top).map(|(p, _)| p).unwrap_or(0);
                    let should_pop = match assoc {
                        Assoc::Left => top_prec >= prec,
                        Assoc::Right => top_prec > prec,
                    };
                    if !should_pop {
                        break;
                    }
                    output.push(token_to_instruction(op_stack.pop().unwrap())?);
                }
                op_stack.push(token);
            }
            Token::LParen => {
                op_stack.push(token);
            }
            Token::RParen => loop {
                match op_stack.last() {
                    None => {
                        return Err(CSGParseError::ShuntingYardError(
                            "Mismatched parentheses".into(),
                        ));
                    }
                    Some(Token::LParen) => {
                        op_stack.pop();
                        break;
                    }
                    _ => output.push(token_to_instruction(op_stack.pop().unwrap())?),
                }
            },
        }
    }

    while let Some(op) = op_stack.pop() {
        if op == Token::LParen {
            return Err(CSGParseError::ShuntingYardError(
                "Mismatched parentheses".into(),
            ));
        }
        output.push(token_to_instruction(op)?);
    }

    Ok(output)
}

fn token_to_instruction(token: Token) -> Result<Instruction, CSGParseError> {
    match token {
        Token::Union => Ok(Instruction::Union),
        Token::Intersection => Ok(Instruction::Intersection),
        Token::Difference => Ok(Instruction::Difference),
        Token::Complement => Ok(Instruction::Complement),
        other => Err(CSGParseError::ShuntingYardError(format!(
            "Unexpected token in operator position: {:?}",
            other
        ))),
    }
}

pub fn parse_csg(input: &str, prim_map: &HashMap<String, usize>) -> Result<FlatCSG, CSGParseError> {
    let tokens = tokenize(input, prim_map)?;
    let instructions = shunting_yard(tokens)?;
    Ok(FlatCSG { instructions })
}

fn lex_token_validate(input: &str) -> IResult<&str, Token> {
    let (input, _) = multispace0(input)?;

    if let Ok((rest, _)) = char::<&str, nom::error::Error<&str>>('(')(input) {
        return Ok((rest, Token::LParen));
    }
    if let Ok((rest, _)) = char::<&str, nom::error::Error<&str>>(')')(input) {
        return Ok((rest, Token::RParen));
    }
    if let Ok((rest, _)) = char::<&str, nom::error::Error<&str>>('+')(input) {
        return Ok((rest, Token::Union));
    }
    if let Ok((rest, _)) = char::<&str, nom::error::Error<&str>>('*')(input) {
        return Ok((rest, Token::Intersection));
    }
    if let Ok((rest, _)) = char::<&str, nom::error::Error<&str>>('-')(input) {
        return Ok((rest, Token::Difference));
    }
    if let Ok((rest, _)) = char::<&str, nom::error::Error<&str>>('!')(input) {
        return Ok((rest, Token::Complement));
    }

    let (rest, _) = parse_identifier(input)?;
    Ok((rest, Token::Primitive(0))) // dummy index
}

fn tokenize_validate(mut input: &str) -> Result<Vec<Token>, CSGParseError> {
    let mut tokens = Vec::new();

    loop {
        let (rest, _) = multispace0::<&str, nom::error::Error<&str>>(input).unwrap();
        if rest.is_empty() {
            break;
        }
        match lex_token_validate(rest) {
            Ok((rest, tok)) => {
                tokens.push(tok);
                input = rest;
            }
            Err(e) => return Err(CSGParseError::LexError(format!("{:?}", e))),
        }
    }

    Ok(tokens)
}

/// validate CSG string without prim_map
pub fn validate_csg_syntax(input: &str) -> Result<(), CSGParseError> {
    let tokens = tokenize_validate(input)?;
    let instructions = shunting_yard(tokens)?;
    validate_csg_instructions(&instructions)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_prim_map() -> HashMap<String, usize> {
        let mut m = HashMap::new();
        m.insert("A".to_string(), 0);
        m.insert("B".to_string(), 1);
        m.insert("C".to_string(), 2);
        m
    }

    #[test]
    fn test_parse_csg_basic_ops() {
        let prim_map = setup_prim_map();

        // A + B (Union)
        let res = parse_csg("A + B", &prim_map).unwrap();
        assert_eq!(res.instructions.len(), 3);
        assert_eq!(res.instructions[0], Instruction::PushPrimitive(0));
        assert_eq!(res.instructions[1], Instruction::PushPrimitive(1));
        assert_eq!(res.instructions[2], Instruction::Union);

        // A * B - C (Intersection then Difference)
        // priority: `* > -` so `(A * B) - C`
        let res = parse_csg("A * B - C", &prim_map).unwrap();
        let expected = vec![
            Instruction::PushPrimitive(0),
            Instruction::PushPrimitive(1),
            Instruction::Intersection,
            Instruction::PushPrimitive(2),
            Instruction::Difference,
        ];
        assert_eq!(res.instructions, expected);
    }

    #[test]
    fn test_parse_csg_precedence_and_parens() {
        let prim_map = setup_prim_map();

        // priority: `()` > `!` > `*` > `+`
        let res = parse_csg("A * (B + C)", &prim_map).unwrap();
        let expected = vec![
            Instruction::PushPrimitive(0),
            Instruction::PushPrimitive(1),
            Instruction::PushPrimitive(2),
            Instruction::Union,
            Instruction::Intersection,
        ];
        assert_eq!(res.instructions, expected);

        // unary operator Complement: !A
        let res = parse_csg("!A", &prim_map).unwrap();
        assert_eq!(
            res.instructions,
            vec![Instruction::PushPrimitive(0), Instruction::Complement]
        );
    }

    #[test]
    fn test_parse_csg_errors() {
        let prim_map = setup_prim_map();

        // undefined identifier
        let res = parse_csg("A + Unknown", &prim_map);
        assert!(matches!(res, Err(CSGParseError::LexError(_))));

        // mismatched parentheses
        let res = parse_csg("(A + B", &prim_map);
        assert!(matches!(res, Err(CSGParseError::ShuntingYardError(_))));
    }

    #[test]
    fn test_validate_csg_syntax_pass() {
        // syntax check without prim_map
        assert!(validate_csg_syntax("A + B * C").is_ok());
        assert!(validate_csg_syntax("!(A - B)").is_ok());
    }

    #[test]
    fn test_validate_csg_syntax_fail() {
        // syntax error: only operator
        assert!(matches!(
            validate_csg_syntax("+"),
            Err(CSGParseError::CSGInstructionValidationError(_))
        ));

        // syntax error: mismatched parentheses
        assert!(matches!(
            validate_csg_syntax("(A + B"),
            Err(CSGParseError::ShuntingYardError(_))
        ));

        // semantic error: not enough operands for operator (detected by validate_csg_instructions)
        // e.g. "A +" passes shunting yard but stack underflows
        assert!(matches!(
            validate_csg_syntax("A +"),
            Err(CSGParseError::CSGInstructionValidationError(_))
        ));
    }

    #[test]
    fn test_complex_expression() {
        let prim_map = setup_prim_map();
        // (A + B) * !(C - A)
        let input = "(A + B) * !(C - A)";
        let res = parse_csg(input, &prim_map).expect("Should parse complex expression");

        let expected = vec![
            Instruction::PushPrimitive(0), // A
            Instruction::PushPrimitive(1), // B
            Instruction::Union,            // +
            Instruction::PushPrimitive(2), // C
            Instruction::PushPrimitive(0), // A
            Instruction::Difference,       // -
            Instruction::Complement,       // !
            Instruction::Intersection,     // *
        ];
        assert_eq!(res.instructions, expected);
    }
}
