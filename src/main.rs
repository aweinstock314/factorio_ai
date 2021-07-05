pub mod lua_parser;

use nom::{error::convert_error, Finish, Parser};
use serde::{Deserialize, Serialize};
use std::{error::Error, fs::File, io::Read};

use lua_parser::{parse_data_extend, LuaObject};
use std::cmp::Ordering;
use std::collections::{HashMap, VecDeque, HashSet};
use std::convert::{TryFrom, TryInto};
use std::iter::FromIterator;

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
    category: String,
    enabled: bool,
    ingredients: Vec<Ingredient>,
    speed: ProductsPerSecond,
    results: Vec<Ingredient>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModuleEffect {
    speed: f64,
    consumption: f64,
    productivity: f64,
    pollution: f64,
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

    // TODO: Parse (avi?)

    // mining-drill.lua
    let mining_speed = HashMap::<ProductId, ProductsPerSecond>::from_iter([
        ("electric-mining-drill".into(), 0.5f64),
        ("burner-mining-drill".into(), 0.25f64),
        ("pumpjack".into(), 1f64),
    ]);

    // item.lua
    let productivity_allowed = HashSet::<String>::from_iter([
        String::from("sulfuric-acid"),
        String::from("basic-oil-processing"),
        String::from("advanced-oil-processing"),
        String::from("coal-liquefaction"),
        String::from("heavy-oil-cracking"),
        String::from("light-oil-cracking"),
        String::from("solid-fuel-from-light-oil"),
        String::from("solid-fuel-from-heavy-oil"),
        String::from("solid-fuel-from-petroleum-gas"),
        String::from("lubricant"),
        String::from("iron-plate"),
        String::from("copper-plate"),
        String::from("steel-plate"),
        String::from("stone-brick"),
        String::from("sulfur"),
        String::from("plastic-bar"),
        String::from("empty-barrel"),
        String::from("uranium-processing"),
        String::from("copper-cable"),
        String::from("iron-stick"),
        String::from("iron-gear-wheel"),
        String::from("electronic-circuit"),
        String::from("advanced-circuit"),
        String::from("processing-unit"),
        String::from("engine-unit"),
        String::from("electric-engine-unit"),
        String::from("uranium-fuel-cell"),
        String::from("explosives"),
        String::from("battery"),
        String::from("flying-robot-frame"),
        String::from("low-density-structure"),
        String::from("rocket-fuel"),
        String::from("nuclear-fuel"),
        String::from("nuclear-fuel-reprocessing"),
        String::from("rocket-control-unit"),
        String::from("rocket-part"),
        String::from("automation-science-pack"),
        String::from("logistic-science-pack"),
        String::from("chemical-science-pack"),
        String::from("military-science-pack"),
        String::from("production-science-pack"),
        String::from("utility-science-pack"),
        String::from("kovarex-enrichment-process")
    ]);

    // item.lua
    let module_bonuses = HashMap::<String, ModuleEffect>::from_iter([
        (String::from("speed-module"), ModuleEffect { speed: 0.2, consumption: 0.5, productivity: 0.0, pollution: 0.0}),
        (String::from("speed-module-2"), ModuleEffect { speed: 0.3, consumption: 0.6, productivity: 0.0, pollution: 0.0}),
        (String::from("speed-module-3"), ModuleEffect { speed: 0.5, consumption: 0.7, productivity: 0.0, pollution: 0.0}),
        (String::from("efficiency-module"), ModuleEffect { speed: 0.0, consumption: -0.3, productivity: 0.0, pollution: 0.0}),
        (String::from("efficiency-module-2"), ModuleEffect { speed: 0.0, consumption: -0.4, productivity: 0.0, pollution: 0.0}),
        (String::from("efficiency-module-3"), ModuleEffect { speed: 0.0, consumption: -0.5, productivity: 0.0, pollution: 0.0}),
        (String::from("productivity-module"), ModuleEffect { speed: -0.05, consumption: 0.4, productivity: 0.04, pollution: 0.05}),
        (String::from("productivity-module-2"), ModuleEffect { speed: -0.1, consumption: 0.6, productivity: 0.06, pollution: 0.07}),
        (String::from("productivity-module-3"), ModuleEffect { speed: -0.15, consumption: 0.8, productivity: 0.1, pollution: 0.1}),
    ]);

    // entities.lua
    let modules_allowed = HashMap::<String, u64>::from_iter([
        (String::from("advanced-crafting"), 4),
        (String::from("centrifuging"), 2),
        (String::from("chemistry"), 3),
        (String::from("crafting"), 4),
        (String::from("crafting-with-fluid"), 4),
        (String::from("oil-processing"), 3),
        (String::from("rocket-building"), 4),
        (String::from("smelting"), 2),
    ]);

    let goal: (ProductId, f64) = ("spidertron".into(), 1f64);

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
                    ingredient.name, product, output_amount, fastest
                );
                let mut modded_rate = speed * (ingredient.amount as f64) / (output_amount as f64);
                if productivity_allowed.contains(&fastest.name) {
                    let modules = vec![
                        String::from("productivity-module-3"); // why settle for anything less
                        *modules_allowed.get(&fastest.category).expect("Unknown category") as usize
                    ];

                    let module_effect: f64 = modules
                        .into_iter()
                        .map(|m| module_bonuses
                            .get(&*m)
                            .expect("Unknown module")
                            .productivity
                        ).sum();

                    modded_rate /= 1f64 + module_effect;
                }
                todo_requirements.push_back((
                    ingredient.name.clone(),
                    modded_rate,
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
