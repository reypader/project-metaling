//! Lifts a usable item's `Script` into an `ItemUse` when the whole script is one heal statement
//! (catalog `scripting/items.md` §5 "Importer notes"):
//! - `itemheal <hp>,<sp>;` where each side is a non-negative integer or `rand(min,max)`;
//! - `percentheal <hp%>,<sp%>;`.
//!
//! Comments (`/* … */` and `// …`) are dropped first. Anything else (a second statement, a
//! condition, a negative or computed value) is not lifted.

use ro_db::ItemUse;

/// The `ItemUse` of `script`, or `None` when it is not exactly one liftable statement.
pub fn lift(script: &str) -> Option<ItemUse> {
    let code = strip_comments(script);
    let statement = code.trim().strip_suffix(';')?.trim();
    if statement.contains(';') {
        return None;
    }
    let (command, args) = statement.split_once(char::is_whitespace)?;
    let (first, second) = split_args(args.trim())?;
    match command {
        "itemheal" => Some(ItemUse::ItemHeal {
            hp: range(first)?,
            sp: range(second)?,
        }),
        "percentheal" => Some(ItemUse::PercentHeal {
            hp: number(first)?.try_into().ok()?,
            sp: number(second)?.try_into().ok()?,
        }),
        _ => None,
    }
}

/// `script` without its comments; an unterminated `/*` drops the rest.
fn strip_comments(script: &str) -> String {
    let mut out = String::with_capacity(script.len());
    let mut rest = script;
    loop {
        let block = rest.find("/*");
        let line = rest.find("//");
        match (block, line) {
            (Some(b), l) if l.is_none_or(|l| b < l) => {
                out.push_str(&rest[..b]);
                match rest[b + 2..].find("*/") {
                    Some(end) => rest = &rest[b + 2 + end + 2..],
                    None => return out,
                }
            }
            (_, Some(l)) => {
                out.push_str(&rest[..l]);
                match rest[l..].find('\n') {
                    Some(end) => rest = &rest[l + end..],
                    None => return out,
                }
            }
            _ => {
                out.push_str(rest);
                return out;
            }
        }
    }
}

/// Splits `a,b` at the comma that is not inside `rand(…)`.
fn split_args(args: &str) -> Option<(&str, &str)> {
    let mut depth = 0u32;
    for (i, c) in args.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.checked_sub(1)?,
            ',' if depth == 0 => {
                let second = args[i + 1..].trim();
                if split_args_has_comma(second) {
                    return None;
                }
                return Some((args[..i].trim(), second));
            }
            _ => {}
        }
    }
    None
}

/// A third top-level argument (`itemheal a,b,char_id`) is not lifted.
fn split_args_has_comma(arg: &str) -> bool {
    let mut depth = 0i32;
    arg.chars().any(|c| {
        match c {
            '(' => depth += 1,
            ')' => depth -= 1,
            _ => {}
        }
        c == ',' && depth == 0
    })
}

/// A constant `n` is `(n, n)`; `rand(a,b)` is `(a, b)` with `a <= b`.
fn range(arg: &str) -> Option<(u32, u32)> {
    if let Some(inner) = arg
        .strip_prefix("rand")
        .map(str::trim_start)
        .and_then(|r| r.strip_prefix('('))
        .and_then(|r| r.strip_suffix(')'))
    {
        let (low, high) = inner.split_once(',')?;
        let (low, high) = (number(low.trim())?, number(high.trim())?);
        return (low <= high).then_some((low, high));
    }
    number(arg).map(|n| (n, n))
}

/// A non-negative decimal literal.
fn number(arg: &str) -> Option<u32> {
    if arg.is_empty() || !arg.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    arg.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes() {
        let heal = |hp, sp| Some(ItemUse::ItemHeal { hp, sp });
        assert_eq!(lift("itemheal rand(10,20),0;\n"), heal((10, 20), (0, 0)));
        assert_eq!(lift("itemheal 7,rand(3,4);"), heal((7, 7), (3, 4)));
        assert_eq!(lift("itemheal rand(1,2),rand(3,4);"), heal((1, 2), (3, 4)));
        assert_eq!(lift(" itemheal  5 , 6 ;"), heal((5, 5), (6, 6)));
        assert_eq!(lift("itemheal 9,0; /* note */"), heal((9, 9), (0, 0)));
        assert_eq!(lift("/* todo */\nitemheal 9,0;"), heal((9, 9), (0, 0)));
        assert_eq!(
            lift("percentheal 25,10; // x"),
            Some(ItemUse::PercentHeal { hp: 25, sp: 10 })
        );
    }

    #[test]
    fn not_lifted() {
        for script in [
            "itemheal 5,0; sc_end SC_POISON;",
            "percentheal 5,0; itemheal 0,5;",
            "if (x) itemheal 5,0;",
            "itemheal -5,0;",
            "itemheal rand(9,1),0;",
            "itemheal 5,0,getcharid(0);",
            "itemheal .@a,0;",
            "itemheal 5,0",
            "percentheal 300,0;",
            "getitem 501,1;",
            "",
        ] {
            assert_eq!(lift(script), None, "{script:?}");
        }
    }
}
