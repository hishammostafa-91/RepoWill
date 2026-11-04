// SPDX-License-Identifier: AGPL-3.0-or-later
use repowill_model::policy::Policy;
use schemars::schema_for;

fn main() {
    let schema = schema_for!(Policy);
    println!("{}", serde_json::to_string_pretty(&schema).unwrap());
}
