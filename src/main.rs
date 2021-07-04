use nom::{
    error::{convert_error, VerboseError},
    Finish,
};
use std::{error::Error, fs::File, io::Read};
use ron::ser::PrettyConfig;
use serde::{Serialize, Deserialize};

pub mod lua_parser;

use lua_parser::{parse_data_extend, LuaObject};
use std::convert::{TryFrom, TryInto};
use std::collections::HashMap;


#[derive(Debug, Clone, Serialize, Deserialize)]
struct Inrgedient {
    name: String,
    amount: u64,
    type_: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Recipe {
    name: String,
    enabled: bool,
    ingredients: Vec<Inrgedient>,
    energy_required: f64,
    results: Vec<Inrgedient>,
}

impl TryFrom<LuaObject> for Inrgedient {
    type Error = String;

    fn try_from(value: LuaObject) -> Result<Self, Self::Error> {
        let array_form = <(String, u64)>::try_from(value.clone());
        let map_form = HashMap::<String, LuaObject>::try_from(value.clone());
        let string_form = String::try_from(value);

        let name;
        let amount;
        let type_;

        match (array_form, map_form, string_form) {
            (Ok((name_, amount_)), Err(_), Err(_)) => {
                name = name_;
                amount = amount_;
                type_ = String::from("item");
            },
            (Err(_), Ok(mut map), Err(_)) => {
                name = map.remove_entry("name")
                    .ok_or("Cannot find field 'name'".into())
                    .and_then(|(_, l)| String::try_from(l))?;
                amount = map.remove_entry("amount")
                    .map_or_else(|| Ok(1), |(_, l)| u64::try_from(l))?;
                type_ = map.remove_entry("type")
                    .map_or_else(|| Ok("item".into()), |(_, l)| String::try_from(l))?;
            },
            (Err(_), Err(_), Ok(name_)) => {
                name = name_;
                amount = 1;
                type_ = "item".into();
            }
            _ => return Err("Cannot decode ingredient".into()),
        }

        Ok(Inrgedient {
            name, amount, type_
        })
    }
}

impl TryFrom<LuaObject> for Recipe {
    type Error = String;

    fn try_from(lua: LuaObject) -> Result<Self, Self::Error> {
        let mut conts: HashMap<String, LuaObject> = lua.try_into()?;

        let name: String = conts.remove_entry("name")
            .ok_or("No entry 'name'".into())
            .and_then(|(_, l)| l.try_into())?;

        println!("{}", name);

        let recipe: Result<HashMap<String, LuaObject>, String> = conts.remove_entry("normal").ok_or("No normal recipe".into()).and_then(|(_, l)| l.try_into());
        let (results, enabled, energy_required, ingredients) = if let Ok(mut recipe) = recipe {
            (
                recipe.remove_entry("results")
                    .map_or_else(
                        || recipe.remove_entry("result")
                            .ok_or("No entry 'result' or 'results'".into())
                            .and_then(|(_, r)| Ok(vec![r.try_into()?])),
                        |(_, l)| l.try_into()
                    )?,
                recipe.remove_entry("enabled")
                    .map_or_else(|| Ok(true), |(_, l)| l.try_into())?,
                recipe.remove_entry("energy_required")
                    .map_or_else(|| Ok(0f64), |(_, l)| l.try_into())?,
                recipe.remove_entry("ingredients")
                    .ok_or("No entry 'ingredients'".into())
                    .and_then(|(_, l)| l.try_into())?,
            )
        } else {
            (
                conts.remove_entry("results")
                    .map_or_else(
                        || conts.remove_entry("result")
                            .ok_or("No entry 'result' or 'results'".into())
                            .and_then(|(_, r)| Ok(vec![r.try_into()?])),
                        |(_, l)| l.try_into()
                    )?,
                conts.remove_entry("enabled")
                    .map_or_else(|| Ok(true), |(_, l)| l.try_into())?,
                conts.remove_entry("energy_required")
                    .map_or_else(|| Ok(0f64), |(_, l)| l.try_into())?,
                conts.remove_entry("ingredients")
                    .ok_or("No entry 'ingredients'".into())
                    .and_then(|(_, l)| l.try_into())?,
            )
        };

        Ok(Recipe {
            name,
            enabled,
            ingredients,
            energy_required,
            results
        })
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut data = File::open("./factorio_headless/factorio/data/base/prototypes/recipe.lua")?;

    let mut string_data = String::new();
    data.read_to_string(&mut string_data)?;

    let recipes: Result<_, VerboseError<_>> = parse_data_extend(&string_data).finish();
    match recipes {
        Ok((_, LuaObject::Array(objs))) => {
            for obj in objs {
                match Recipe::try_from(obj.clone()) {
                    Ok(recipe) => println!("{}", ron::ser::to_string_pretty(&recipe, PrettyConfig::default())?),
                    Err(e) => println!("{} {}", e, ron::ser::to_string_pretty(&obj, PrettyConfig::default())?),
                }
            }
        },
        Err(e) => println!("{}", convert_error(&*string_data, e)),
        _ => println!("Bad parse type"),
    }

    Ok(())
}
