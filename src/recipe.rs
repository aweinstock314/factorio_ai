use std::convert::{TryFrom, TryInto};
use crate::lua_parser::LuaObject;
use std::collections::HashMap;
use serde::{Deserialize, Serialize};

pub type ProductsPerSecond = f64;
pub type ProductId = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ingredient {
    pub name: ProductId,
    pub amount: i64,
    pub type_: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recipe {
    pub name: ProductId,
    pub category: String,
    pub enabled: bool,
    pub ingredients: Vec<Ingredient>,
    pub speed: ProductsPerSecond,
    pub results: Vec<Ingredient>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecipeMap(pub HashMap<ProductId, Vec<Recipe>>);

impl TryFrom<LuaObject> for Ingredient {
    type Error = String;

    fn try_from(value: LuaObject) -> Result<Self, Self::Error> {
        let array_form = <(String, i64)>::try_from(value.clone());
        let map_form = HashMap::<String, LuaObject>::try_from(value.clone());

        let name;
        let amount;
        let type_;

        match (array_form, map_form) {
            (Ok((name_, amount_)), Err(_)) => {
                name = name_;
                amount = amount_;
                type_ = String::from("item");
            }
            (Err(_), Ok(mut map)) => {
                name = map
                    .remove_entry("name")
                    .ok_or("Cannot find field 'name'".into())
                    .and_then(|(_, l)| String::try_from(l))?;
                amount = map
                    .remove_entry("amount")
                    .map_or_else(|| Ok(1), |(_, l)| i64::try_from(l))?;
                type_ = map
                    .remove_entry("type")
                    .map_or_else(|| Ok("item".into()), |(_, l)| String::try_from(l))?;
            }
            _ => return Err("Cannot decode ingredient".into()),
        }

        Ok(Ingredient {
            name,
            amount,
            type_,
        })
    }
}

impl TryFrom<LuaObject> for Recipe {
    type Error = String;

    fn try_from(lua: LuaObject) -> Result<Self, Self::Error> {
        let mut conts: HashMap<String, LuaObject> = lua.try_into()?;

        let name: String = conts
            .remove_entry("name")
            .ok_or("No entry 'name'".into())
            .and_then(|(_, l)| l.try_into())?;

        let category: String = conts
            .remove_entry("category")
            .map_or_else(|| Ok(String::from("crafting")), |(_, l)| l.try_into())?;

        let recipe: Result<HashMap<String, LuaObject>, String> = conts
            .remove_entry("normal")
            .ok_or("No normal recipe".into())
            .and_then(|(_, l)| l.try_into());
        let (results, enabled, energy_required, ingredients) = if let Ok(mut recipe) = recipe {
            (
                recipe.remove_entry("results").map_or_else(
                    || {
                        recipe
                            .remove_entry("result")
                            .ok_or("No entry 'result' or 'results'".into())
                            .and_then(|(_, r)| {
                                recipe.remove_entry("result_count").map_or_else(
                                    || Ok((String::try_from(r.clone())?, 1i64)),
                                    |(_, c)| Ok((String::try_from(r.clone())?, i64::try_from(c)?)),
                                )
                            })
                            .and_then(|(r, c)| {
                                Ok(vec![Ingredient {
                                    name: r,
                                    amount: c,
                                    type_: "item".into(),
                                }])
                            })
                    },
                    |(_, l)| l.try_into(),
                )?,
                recipe
                    .remove_entry("enabled")
                    .map_or_else(|| Ok(true), |(_, l)| l.try_into())?,
                recipe
                    .remove_entry("energy_required")
                    .map_or_else(|| Ok(1f64), |(_, l)| l.try_into())?,
                recipe
                    .remove_entry("ingredients")
                    .ok_or("No entry 'ingredients'".into())
                    .and_then(|(_, l)| l.try_into())?,
            )
        } else {
            (
                conts.remove_entry("results").map_or_else(
                    || {
                        conts
                            .remove_entry("result")
                            .ok_or("No entry 'result' or 'results'".into())
                            .and_then(|(_, r)| {
                                conts.remove_entry("result_count").map_or_else(
                                    || Ok((String::try_from(r.clone())?, 1i64)),
                                    |(_, c)| Ok((String::try_from(r.clone())?, i64::try_from(c)?)),
                                )
                            })
                            .and_then(|(r, c)| {
                                Ok(vec![Ingredient {
                                    name: r,
                                    amount: c,
                                    type_: "item".into(),
                                }])
                            })
                    },
                    |(_, l)| l.try_into(),
                )?,
                conts
                    .remove_entry("enabled")
                    .map_or_else(|| Ok(true), |(_, l)| l.try_into())?,
                conts
                    .remove_entry("energy_required")
                    .map_or_else(|| Ok(1f64), |(_, l)| l.try_into())?,
                conts
                    .remove_entry("ingredients")
                    .ok_or("No entry 'ingredients'".into())
                    .and_then(|(_, l)| l.try_into())?,
            )
        };

        Ok(Recipe {
            name,
            category,
            enabled,
            ingredients,
            speed: 1f64 / energy_required,
            results,
        })
    }
}

impl RecipeMap {
    pub fn new(recipes: Vec<Recipe>) -> Self {
        let mut recipe_map = HashMap::<ProductId, Vec<Recipe>>::new();
        for recipe in recipes {
            for output in &recipe.results {
                if let Some(m) = recipe_map.get_mut(&output.name) {
                    m.push(recipe.clone());
                } else {
                    recipe_map.insert(output.name.clone(), vec![recipe.clone()]);
                }
            }
        }

        RecipeMap(recipe_map)
    }
}
