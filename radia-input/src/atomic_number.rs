use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::fmt;

fn symbol_to_atomic_number(symbol: &str) -> Option<usize> {
    const ELEMENTS: &[&str] = &[
        "H", "He", "Li", "Be", "B", "C", "N", "O", "F", "Ne", "Na", "Mg", "Al", "Si", "P", "S",
        "Cl", "Ar", "K", "Ca", "Sc", "Ti", "V", "Cr", "Mn", "Fe", "Co", "Ni", "Cu", "Zn", "Ga",
        "Ge", "As", "Se", "Br", "Kr", "Rb", "Sr", "Y", "Zr", "Nb", "Mo", "Tc", "Ru", "Rh", "Pd",
        "Ag", "Cd", "In", "Sn", "Sb", "Te", "I", "Xe", "Cs", "Ba", "La", "Ce", "Pr", "Nd", "Pm",
        "Sm", "Eu", "Gd", "Tb", "Dy", "Ho", "Er", "Tm", "Yb", "Lu", "Hf", "Ta", "W", "Re", "Os",
        "Ir", "Pt", "Au", "Hg", "Tl", "Pb", "Bi", "Po", "At", "Rn", "Fr", "Ra", "Ac", "Th", "Pa",
        "U", "Np", "Pu", "Am", "Cm", "Bk", "Cf", "Es", "Fm", "Md", "No", "Lr", "Rf", "Db", "Sg",
        "Bh", "Hs", "Mt", "Ds", "Rg", "Cn", "Nh", "Fl", "Mc", "Lv", "Ts", "Og",
    ];
    ELEMENTS.iter().position(|&s| s == symbol).map(|i| i + 1)
}

struct AtomicNumberKey(usize);

impl<'de> Deserialize<'de> for AtomicNumberKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct KeyVisitor;

        impl<'de> Visitor<'de> for KeyVisitor {
            type Value = AtomicNumberKey;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(
                    f,
                    "an element symbol (e.g. \"Fe\") or atomic number (e.g. 26)"
                )
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<Self::Value, E> {
                let z = v as usize;
                if z == 0 || z > 118 {
                    return Err(E::custom(format!(
                        "atomic number {z} is out of range (1-118)"
                    )));
                }
                Ok(AtomicNumberKey(z))
            }

            fn visit_i64<E: de::Error>(self, v: i64) -> Result<Self::Value, E> {
                if v <= 0 || v > 118 {
                    return Err(E::custom(format!(
                        "atomic number {v} is out of range (1-118)"
                    )));
                }
                Ok(AtomicNumberKey(v as usize))
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                let z = symbol_to_atomic_number(v)
                    .ok_or_else(|| E::custom(format!("unknown element symbol: {v}")))?;
                Ok(AtomicNumberKey(z))
            }
        }

        deserializer.deserialize_any(KeyVisitor)
    }
}

pub fn deserialize_composition<'de, D>(deserializer: D) -> Result<HashMap<usize, f32>, D::Error>
where
    D: Deserializer<'de>,
{
    struct CompositionVisitor;

    impl<'de> Visitor<'de> for CompositionVisitor {
        type Value = HashMap<usize, f32>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "a map of element symbols or atomic numbers to fractions")
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut result = HashMap::new();
            while let Some(AtomicNumberKey(z)) = map.next_key::<AtomicNumberKey>()? {
                let frac: f32 = map.next_value()?;
                if result.insert(z, frac).is_some() {
                    return Err(de::Error::custom(format!(
                        "duplicate element: atomic number {z}"
                    )));
                }
            }
            Ok(result)
        }
    }

    deserializer.deserialize_map(CompositionVisitor)
}

/// garde validation function
pub fn validate_composition(map: &HashMap<usize, f32>, _ctx: &()) -> garde::Result {
    let sum: f32 = map.values().sum();
    if (sum - 1.0).abs() > 1e-3 {
        return Err(garde::Error::new(format!(
            "fractions must sum to 1.0, got {sum:.6}"
        )));
    }
    for (&z, &frac) in map {
        if frac <= 0.0 {
            return Err(garde::Error::new(format!(
                "fraction for Z={z} must be positive, got {frac}"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_map(pairs: &[(usize, f32)]) -> HashMap<usize, f32> {
        pairs.iter().cloned().collect()
    }

    #[test]
    fn test_valid_single_element() {
        let map = make_map(&[(1, 1.0)]);
        assert!(validate_composition(&map, &()).is_ok());
    }

    #[test]
    fn test_valid_two_elements() {
        let map = make_map(&[(1, 0.111898), (8, 0.888102)]);
        assert!(validate_composition(&map, &()).is_ok());
    }

    #[test]
    fn test_valid_sum_within_tolerance() {
        let map = make_map(&[(6, 0.3334), (1, 0.3333), (8, 0.3333)]);
        assert!(validate_composition(&map, &()).is_ok());
    }

    #[test]
    fn test_invalid_sum_too_low() {
        let map = make_map(&[(1, 0.3), (8, 0.3)]);
        let err = validate_composition(&map, &()).unwrap_err();
        assert!(err.to_string().contains("sum"));
    }

    #[test]
    fn test_invalid_sum_too_high() {
        let map = make_map(&[(1, 0.7), (8, 0.7)]);
        let err = validate_composition(&map, &()).unwrap_err();
        assert!(err.to_string().contains("sum"));
    }

    #[test]
    fn test_invalid_zero_fraction() {
        let map = make_map(&[(1, 0.0), (8, 1.0)]);
        let err = validate_composition(&map, &()).unwrap_err();
        assert!(err.to_string().contains("positive"));
    }

    #[test]
    fn test_invalid_negative_fraction() {
        let map = make_map(&[(1, -0.1), (8, 1.1)]);
        let err = validate_composition(&map, &()).unwrap_err();
        assert!(err.to_string().contains("positive"));
    }
}
