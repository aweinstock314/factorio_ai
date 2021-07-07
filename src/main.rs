pub mod lua_parser;
pub mod recipe;

use nom::{error::convert_error, Finish, Parser};
use petgraph::Graph;
use serde::{Deserialize, Serialize};
use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet, VecDeque},
    convert::{TryFrom, TryInto},
    error::Error,
    fs::File,
    io::{Read, Write},
    iter::FromIterator,
};

use lua_parser::{parse_data_extend, LuaContext, LuaObject};
use crate::recipe::{ProductId, ProductsPerSecond, Recipe, RecipeMap};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModuleEffect {
    speed: f64,
    consumption: f64,
    productivity: f64,
    pollution: f64,
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut data = File::open("./factorio_headless/factorio/data/base/prototypes/recipe.lua")?;

    let mut string_data = String::new();
    data.read_to_string(&mut string_data)?;

    let raw_recipes = parse_data_extend(&string_data)
        .finish()
        .map_err(|e| convert_error(&*string_data, e).into())
        .and_then(|(_, objs)| Vec::<Recipe>::try_from(objs.simplify()))?;

    let recipe_map = RecipeMap::new(raw_recipes);

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
        String::from("kovarex-enrichment-process"),
    ]);

    // item.lua
    let module_bonuses = HashMap::<String, ModuleEffect>::from_iter([
        (
            String::from("speed-module"),
            ModuleEffect {
                speed: 0.2,
                consumption: 0.5,
                productivity: 0.0,
                pollution: 0.0,
            },
        ),
        (
            String::from("speed-module-2"),
            ModuleEffect {
                speed: 0.3,
                consumption: 0.6,
                productivity: 0.0,
                pollution: 0.0,
            },
        ),
        (
            String::from("speed-module-3"),
            ModuleEffect {
                speed: 0.5,
                consumption: 0.7,
                productivity: 0.0,
                pollution: 0.0,
            },
        ),
        (
            String::from("efficiency-module"),
            ModuleEffect {
                speed: 0.0,
                consumption: -0.3,
                productivity: 0.0,
                pollution: 0.0,
            },
        ),
        (
            String::from("efficiency-module-2"),
            ModuleEffect {
                speed: 0.0,
                consumption: -0.4,
                productivity: 0.0,
                pollution: 0.0,
            },
        ),
        (
            String::from("efficiency-module-3"),
            ModuleEffect {
                speed: 0.0,
                consumption: -0.5,
                productivity: 0.0,
                pollution: 0.0,
            },
        ),
        (
            String::from("productivity-module"),
            ModuleEffect {
                speed: -0.05,
                consumption: 0.4,
                productivity: 0.04,
                pollution: 0.05,
            },
        ),
        (
            String::from("productivity-module-2"),
            ModuleEffect {
                speed: -0.1,
                consumption: 0.6,
                productivity: 0.06,
                pollution: 0.07,
            },
        ),
        (
            String::from("productivity-module-3"),
            ModuleEffect {
                speed: -0.15,
                consumption: 0.8,
                productivity: 0.1,
                pollution: 0.1,
            },
        ),
    ]);

    // entities.lua
    let modules_allowed = HashMap::<String, i64>::from_iter([
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

    let mut graph = Graph::new();
    let mut nodes = HashMap::new();
    let mut requirements = HashMap::new();
    let mut todo_requirements = VecDeque::new();
    todo_requirements.push_back(goal.clone()); // now this is an api i can get behind

    // find a recipe in the map to make this
    while !todo_requirements.is_empty() {
        let (product, speed) = todo_requirements.pop_front().unwrap();
        if let Some(recipes) = recipe_map.0.get(&product) {
            let product_node = *nodes
                .entry(product.clone())
                .or_insert_with(|| graph.add_node(product.clone()));
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
                let ingredient_node = *nodes
                    .entry(ingredient.name.clone())
                    .or_insert_with(|| graph.add_node(ingredient.name.clone()));
                graph.update_edge(ingredient_node, product_node, ());
                let mut modded_rate = speed * (ingredient.amount as f64) / (output_amount as f64);
                if productivity_allowed.contains(&fastest.name) {
                    let modules = vec![
                        String::from("productivity-module-3"); // why settle for anything less
                        *modules_allowed.get(&fastest.category).expect("Unknown category") as usize
                    ];

                    let module_effect: f64 = modules
                        .into_iter()
                        .map(|m| {
                            module_bonuses
                                .get(&*m)
                                .expect("Unknown module")
                                .productivity
                        })
                        .sum();
                    // modded_rate /= 1f64 + module_effect;
                }
                todo_requirements.push_back((ingredient.name.clone(), modded_rate));
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

    {
        use petgraph::dot::{Config, Dot};
        let mut f = File::create("spidertron.dot")?;
        write!(f, "{:?}", Dot::with_config(&graph, &[Config::EdgeNoLabel]))?;
    }

    // println!("{}", ron::ser::to_string_pretty(&recipe_map, PrettyConfig::default())?);

    Ok(())
}

#[test]
fn parse_item() -> Result<(), Box<dyn Error>> {
    let mut data = File::open("./factorio_headless/factorio/data/base/prototypes/item.lua")?;
    let mut string_data = String::new();
    data.read_to_string(&mut string_data)?;
    let mut ctx = LuaContext::new();
    let e = ctx
        .parse_all::<nom::error::VerboseError<_>>(&string_data)
        .finish();
    //println!("{:?}", ctx);
    if let Err(e) = e {
        panic!("{}", convert_error(&*string_data, e));
    }
    println!(
        "{}",
        ron::ser::to_string_pretty(&ctx, ron::ser::PrettyConfig::default())?
    );
    Ok(())
}

#[test]
fn parse_technology() -> Result<(), Box<dyn Error>> {
    let mut data = File::open("./factorio_headless/factorio/data/base/prototypes/technology.lua")?;
    let mut string_data = String::new();
    data.read_to_string(&mut string_data)?;
    let mut ctx = LuaContext::new();
    let e = ctx
        .parse_all::<nom::error::VerboseError<_>>(&string_data)
        .finish();
    //println!("{:?}", ctx);
    if let Err(e) = e {
        panic!("{}", convert_error(&*string_data, e));
    }
    println!(
        "{}",
        ron::ser::to_string_pretty(&ctx, ron::ser::PrettyConfig::default())?
    );
    Ok(())
}
