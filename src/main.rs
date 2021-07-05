pub mod lua_parser;

use nom::{error::convert_error, Finish};
use serde::{Deserialize, Serialize};
use std::{error::Error, fs::File, io::Read};

use lua_parser::{parse_data_extend, LuaObject};
use std::cmp::Ordering;
use std::collections::{HashMap, VecDeque};
use std::convert::{TryFrom, TryInto};

type ProductsPerSecond = f64;
type ProductId = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Ingredient {
    name: ProductId,
    amount: u64,
    type_: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Recipe {
    name: ProductId,
    enabled: bool,
    ingredients: Vec<Ingredient>,
    speed: ProductsPerSecond,
    results: Vec<Ingredient>,
}

impl TryFrom<LuaObject> for Ingredient {
    type Error = String;

    fn try_from(value: LuaObject) -> Result<Self, Self::Error> {
        let array_form = <(String, u64)>::try_from(value.clone());
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
                    .map_or_else(|| Ok(1), |(_, l)| u64::try_from(l))?;
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

        println!("{}", name);

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
                                    || Ok((String::try_from(r.clone())?, 1u64)),
                                    |(_, c)| Ok((String::try_from(r.clone())?, u64::try_from(c)?)),
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
                    .map_or_else(|| Ok(0f64), |(_, l)| l.try_into())?,
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
                                    || Ok((String::try_from(r.clone())?, 1u64)),
                                    |(_, c)| Ok((String::try_from(r.clone())?, u64::try_from(c)?)),
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
                    .map_or_else(|| Ok(0f64), |(_, l)| l.try_into())?,
                conts
                    .remove_entry("ingredients")
                    .ok_or("No entry 'ingredients'".into())
                    .and_then(|(_, l)| l.try_into())?,
            )
        };

        Ok(Recipe {
            name,
            enabled,
            ingredients,
            speed: 1f64 / energy_required,
            results,
        })
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut data = File::open("./factorio_headless/factorio/data/base/prototypes/recipe.lua")?;

    let mut string_data = String::new();
    data.read_to_string(&mut string_data)?;

    let raw_recipes = parse_data_extend(&string_data)
        .finish()
        .map_err(|e| convert_error(&*string_data, e).into())
        .and_then(|(_, objs)| Vec::<Recipe>::try_from(objs))?;

    let mut recipe_map = HashMap::<ProductId, Vec<Recipe>>::new();
    for recipe in raw_recipes {
        for output in &recipe.results {
            if let Some(m) = recipe_map.get_mut(&output.name) {
                m.push(recipe.clone());
            } else {
                recipe_map.insert(output.name.clone(), vec![recipe.clone()]);
            }
        }
    }

    let mut mining_speed = HashMap::<ProductId, ProductsPerSecond>::new();
    mining_speed.insert("electric-mining-drill".into(), 0.5f64);
    mining_speed.insert("burner-mining-drill".into(), 0.25f64);
    mining_speed.insert("pumpjack".into(), 1f64);

    let goal: (ProductId, f64) = ("speed-module".into(), 1f64);

    let mut requirements = HashMap::new();
    let mut todo_requirements = VecDeque::new();
    todo_requirements.push_back(goal.clone()); // now this is an api i can get behind

    // find a recipe in the map to make this
    while !todo_requirements.is_empty() {
        let (product, speed) = todo_requirements.pop_front().unwrap();
        if let Some(recipes) = recipe_map.get(&product) {
            // Find the fastest
            let fastest = recipes
                .iter()
                .min_by(|&a, &b| a.speed.partial_cmp(&b.speed).unwrap_or(Ordering::Equal))
                .expect("Recipes should have entries");

            let output_amount = fastest
                .results
                .iter()
                .filter_map(|res| {
                    if res.name == product {
                        Some(res.amount)
                    } else {
                        None
                    }
                })
                .next()
                .expect("Recipe should have product as a result");

            for ingredient in &fastest.ingredients {
                println!(
                    "Using {} to make {} x {} from {:?}",
                    ingredient.name, product, output_amount, fastest.ingredients
                );
                todo_requirements.push_back((
                    ingredient.name.clone(),
                    speed * (ingredient.amount as f64) / (output_amount as f64),
                ));
            }
        } else {
            if let Some(req) = requirements.get_mut(&product) {
                *req += speed;
            } else {
                requirements.insert(product, speed);
            }
        }
    }

    println!("To make {} @ {}/sec you need:", goal.0, goal.1);
    for (product, speed) in requirements {
        println!("    {} @ {}/sec", product, speed);
    }

    // println!("{}", ron::ser::to_string_pretty(&recipe_map, PrettyConfig::default())?);

    Ok(())
}
