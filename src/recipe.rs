use crate::lua_parser::LuaObject;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};

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

pub trait ConversionExt {
    type Index: ?Sized;
    fn field<'a, T: TryFrom<LuaObject, Error = String>>(
        &mut self,
        index: &Self::Index,
    ) -> Result<T, T::Error>;
}

impl ConversionExt for HashMap<String, LuaObject> {
    type Index = str;
    fn field<T: TryFrom<LuaObject, Error = String>>(&mut self, index: &str) -> Result<T, T::Error> {
        self.remove_entry(index)
            .ok_or_else(|| format!("Couldn't find key {:?} in {:?}", index, self.keys()))
            .and_then(|(_, x)| T::try_from(x))
    }
}

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
                name = map.field("name")?;
                amount = map.field("amount").unwrap_or(1);
                type_ = map.field("type").unwrap_or_else(|_| "item".into());
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

        let name: String = conts.field("name")?;

        let category: String = conts
            .field("category")
            .unwrap_or_else(|_| "crafting".into());

        let recipe: Result<HashMap<String, LuaObject>, String> = conts.field("normal");

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
                recipe.field("enabled").unwrap_or(true),
                recipe.field("energy_required").unwrap_or(1.0),
                recipe.field("ingredients")?,
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
                conts.field("enabled").unwrap_or(true),
                conts.field("energy_required").unwrap_or(1.0),
                conts.field("ingredients")?,
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
