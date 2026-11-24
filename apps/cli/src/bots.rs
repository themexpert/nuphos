//! `nuphos bots`: list your Bots in the team, create one, or set a Bot's
//! standing instructions.

use std::io::Read;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};

use crate::cli::{row, Ctx};
use crate::render::clean;
use crate::shared::{self, label};

/// The backend's title limit, counted in UTF-16 units like zod's `max`.
const MAX_TITLE_UNITS: usize = 120;
/// The backend's limit on standing instructions, counted the same way.
const MAX_INSTRUCTIONS_UNITS: usize = 8000;

#[derive(Debug, PartialEq)]
enum Action {
    List,
    Create(String),
    Instruct(String),
}

/// A bare name must not create or change anything, so `--create` or
/// `--instructions` is required for one.
fn plan(name: Option<String>, create: bool, instruct: bool) -> Result<Action> {
    match (name, create, instruct) {
        (None, _, _) => Ok(Action::List),
        (Some(name), true, _) => Ok(Action::Create(name)),
        (Some(name), false, true) => Ok(Action::Instruct(name)),
        (Some(name), false, false) => bail!(
            "`nuphos bots` only lists Bots; to create one named '{}' run `nuphos bots '{}' --create`.",
            clean(&name),
            clean(&name)
        ),
    }
}

/// The shape of an avatar name such as `01-vermeer-girl`; the backend knows which exist.
fn avatar_shape_ok(avatar: &str) -> bool {
    let bytes = avatar.as_bytes();
    bytes.len() >= 4
        && bytes.len() <= 43
        && bytes[..2].iter().all(u8::is_ascii_digit)
        && bytes[2] == b'-'
        && bytes[3..].iter().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-')
}

/// The `POST /agent/bots` body; the team goes in the query. The runtime is the
/// one a new chat would use.
fn create_body(title: &str, avatar: Option<&str>, runtime: Option<&Value>) -> Result<Value> {
    let title = title.trim();
    if title.is_empty() || title.encode_utf16().count() > MAX_TITLE_UNITS {
        bail!("A Bot's name must be 1 to {MAX_TITLE_UNITS} characters.");
    }
    let mut body = json!({ "title": title });
    if let Some(avatar) = avatar {
        if !avatar_shape_ok(avatar) {
            bail!(
                "'{}' is not an avatar name; they look like 01-vermeer-girl (see the desktop's Bot picker).",
                clean(avatar)
            );
        }
        body["avatar"] = json!(avatar);
    }
    if let Some(r) = runtime {
        body["runtimeId"] = r["id"].clone();
        body["agentRuntime"] = r["provider"].clone();
    }
    Ok(body)
}

/// The text the backend would store; text over its limit is refused here.
fn instructions_text(raw: &str) -> Result<String> {
    let text = raw.trim();
    let units = text.encode_utf16().count();
    if units > MAX_INSTRUCTIONS_UNITS {
        bail!("A Bot's instructions must be at most {MAX_INSTRUCTIONS_UNITS} characters; these are {units}.");
    }
    Ok(text.to_string())
}

/// The instructions in FILE, or on stdin for `-`.
fn read_instructions(file: &str) -> Result<String> {
    let raw = if file == "-" {
        let mut text = String::new();
        std::io::stdin().read_to_string(&mut text).context("Could not read the instructions from stdin")?;
        text
    } else {
        std::fs::read_to_string(file)
            .with_context(|| format!("Could not read the instructions from {}", clean(file)))?
    };
    instructions_text(&raw)
}

/// The Bot with session id `wanted`, or the one titled `wanted` (ignoring
/// case); a title two Bots share is an error.
fn pick_bot(bots: &[Value], wanted: &str) -> Result<Value> {
    if let Some(bot) = bots.iter().find(|b| b["sessionId"] == wanted) {
        return Ok(bot.clone());
    }
    let lower = wanted.to_lowercase();
    let same: Vec<&Value> =
        bots.iter().filter(|b| b["title"].as_str().is_some_and(|t| t.to_lowercase() == lower)).collect();
    match same.as_slice() {
        [bot] => Ok((*bot).clone()),
        [] => bail!("No Bot named '{}'. `nuphos bots` lists them.", clean(wanted)),
        _ => {
            let ids: Vec<String> = same.iter().map(|b| clean(b["sessionId"].as_str().unwrap_or("?"))).collect();
            bail!("{} Bots are named '{}'. Use an id: {}", same.len(), clean(wanted), ids.join(", "))
        }
    }
}

/// `nuphos bots [NAME [--create] [--instructions FILE]] [--avatar A] [--agent RUNTIME]`.
pub async fn bots(
    ctx: &Ctx,
    name: Option<String>,
    create: bool,
    instructions: Option<String>,
    avatar: Option<String>,
    runtime: Option<String>,
) -> Result<()> {
    let team = ctx.team["id"].as_str().unwrap_or_default().to_string();
    let action = plan(name, create, instructions.is_some())?;
    // Read the file first, so a missing one does not leave a new Bot without its instructions.
    let instructions = instructions.as_deref().map(read_instructions).transpose()?;
    match action {
        Action::List => list(ctx, &team).await,
        Action::Create(name) => create_bot(ctx, &team, &name, avatar.as_deref(), runtime, instructions).await,
        Action::Instruct(name) => {
            let (bots, _) = ctx.api.bots(&team).await?;
            let bot = pick_bot(&bots, &name)?;
            let id = bot["sessionId"].as_str().unwrap_or_default();
            let text = instructions.unwrap_or_default();
            ctx.api.set_bot_instructions(&team, id, &text).await?;
            if ctx.json {
                println!("{}", serde_json::to_string_pretty(&json!({ "sessionId": id, "instructions": text }))?);
            } else {
                println!("{}", instructions_saved(&shared::conversation_title(&bot), &text));
            }
            Ok(())
        }
    }
}

fn instructions_saved(bot: &str, text: &str) -> String {
    if text.is_empty() {
        format!("Cleared the standing instructions of {bot}.")
    } else {
        format!("Saved the standing instructions of {bot} ({} characters).", text.encode_utf16().count())
    }
}

async fn list(ctx: &Ctx, team: &str) -> Result<()> {
    let (bots, more) = ctx.api.bots(team).await?;
    if ctx.json {
        println!("{}", serde_json::to_string_pretty(&bots)?);
    } else {
        for b in &bots {
            row(
                false,
                &[
                    b["sessionId"].as_str().unwrap_or_default(),
                    &shared::conversation_time(b),
                    &shared::conversation_title(b),
                    b["avatar"].as_str().unwrap_or_default(),
                ],
            );
        }
    }
    if more {
        eprintln!("nuphos: showing the first {} Bots; there are more.", bots.len());
    }
    Ok(())
}

async fn create_bot(
    ctx: &Ctx,
    team: &str,
    name: &str,
    avatar: Option<&str>,
    runtime: Option<String>,
    instructions: Option<String>,
) -> Result<()> {
    let runtimes = shared::active_runtimes(ctx.api.runtimes(team).await?);
    let runtime = match runtime {
        Some(wanted) => Some(shared::find(&runtimes, &wanted, "runtime")?),
        None => shared::default_runtime(&runtimes, &ctx.prefs, team, &ctx.me_id),
    };
    let body = create_body(name, avatar, runtime.as_ref())?;
    let mut bot = ctx.api.create_bot(team, &body).await.map_err(|e| {
        if e.status == 400 && avatar.is_some() {
            anyhow!("{e}. Avatar names are the Bot picker's faces, such as 01-vermeer-girl.")
        } else {
            e.into()
        }
    })?;
    let id = bot["sessionId"].as_str().ok_or_else(|| anyhow!("The server returned no Bot id."))?.to_string();
    // A new Bot has none to clear.
    let instructions = instructions.filter(|text| !text.is_empty());
    if let Some(text) = &instructions {
        ctx.api.set_bot_instructions(team, &id, text).await.with_context(|| {
            format!(
                "Created Bot {}, but could not save its instructions; retry with `nuphos bots {} --instructions FILE`",
                clean(&id),
                clean(&id)
            )
        })?;
        bot["instructions"] = json!(text);
    }
    if ctx.json {
        println!("{}", serde_json::to_string_pretty(&bot)?);
        return Ok(());
    }
    let on = runtime.as_ref().map_or_else(|| "the team's default runtime".to_string(), label);
    println!(
        "Created Bot {} ({}) on {on}; it runs with Full Access.",
        clean(bot["title"].as_str().unwrap_or(name)),
        clean(&id)
    );
    if let Some(text) = &instructions {
        println!("Saved its standing instructions ({} characters).", text.encode_utf16().count());
    }
    println!("Talk to it with: nuphos exec --session {} \"...\"", clean(&id));
    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::*;
    use crate::{Args, Command};

    fn parse(args: &[&str]) -> Command {
        let mut argv = vec!["nuphos"];
        argv.extend(args);
        Args::try_parse_from(argv).unwrap().command.unwrap()
    }

    #[test]
    fn create_takes_a_name_avatar_and_agent() {
        match parse(&["bot", "Scout", "--create", "--avatar", "07-ada", "--agent", "Codex"]) {
            Command::Bots { name, create, instructions, avatar, runtime } => {
                assert_eq!(name.as_deref(), Some("Scout"));
                assert!(create);
                assert_eq!(instructions, None);
                assert_eq!(avatar.as_deref(), Some("07-ada"));
                assert_eq!(runtime.as_deref(), Some("Codex"));
            }
            _ => panic!("not bots"),
        }
    }

    #[test]
    fn options_for_a_new_bot_need_create() {
        assert!(Args::try_parse_from(["nuphos", "bots", "Scout", "--avatar", "07-ada"]).is_err());
        assert!(Args::try_parse_from(["nuphos", "bots", "--create"]).is_err());
    }

    #[test]
    fn instructions_need_a_bot_and_go_with_create_or_alone() {
        assert!(Args::try_parse_from(["nuphos", "bots", "--instructions", "rules.md"]).is_err());
        for args in [
            &["bots", "Scout", "--instructions", "rules.md"][..],
            &["bots", "Scout", "--create", "--instructions", "-"],
        ] {
            match parse(args) {
                Command::Bots { name, instructions, .. } => {
                    assert_eq!(name.as_deref(), Some("Scout"));
                    assert!(instructions.is_some());
                }
                _ => panic!("not bots"),
            }
        }
    }

    #[test]
    fn a_bare_name_creates_nothing() {
        assert_eq!(plan(None, false, false).unwrap(), Action::List);
        assert_eq!(plan(Some("Scout".into()), true, false).unwrap(), Action::Create("Scout".into()));
        assert_eq!(plan(Some("Scout".into()), true, true).unwrap(), Action::Create("Scout".into()));
        assert_eq!(plan(Some("Scout".into()), false, true).unwrap(), Action::Instruct("Scout".into()));
        let error = plan(Some("Scout".into()), false, false).unwrap_err().to_string();
        assert!(error.contains("nuphos bots 'Scout' --create"), "{error}");
    }

    #[test]
    fn instructions_are_trimmed_and_held_to_the_backend_limit() {
        assert_eq!(instructions_text("\n  Answer tickets.\n\n").unwrap(), "Answer tickets.");
        assert_eq!(instructions_text(" \n").unwrap(), "");
        assert!(instructions_text(&"x".repeat(MAX_INSTRUCTIONS_UNITS)).is_ok());
        assert!(instructions_text(&format!("  {}  ", "x".repeat(MAX_INSTRUCTIONS_UNITS))).is_ok());
        let error = instructions_text(&"x".repeat(MAX_INSTRUCTIONS_UNITS + 1)).unwrap_err().to_string();
        assert!(error.contains("8001"), "{error}");
        // One emoji is two UTF-16 units.
        assert!(instructions_text(&"\u{1F600}".repeat(4000)).is_ok());
        assert!(instructions_text(&"\u{1F600}".repeat(4001)).is_err());
    }

    #[test]
    fn a_missing_instructions_file_is_an_error() {
        let error = read_instructions("/nonexistent/rules.md").unwrap_err().to_string();
        assert!(error.contains("/nonexistent/rules.md"), "{error}");
    }

    #[test]
    fn a_bot_is_picked_by_id_or_by_a_title_only_one_bot_has() {
        let bots = [
            json!({ "sessionId": "s1", "title": "Support" }),
            json!({ "sessionId": "s2", "title": "Release" }),
            json!({ "sessionId": "s3", "title": "release" }),
        ];
        assert_eq!(pick_bot(&bots, "s3").unwrap()["sessionId"], "s3");
        assert_eq!(pick_bot(&bots, "SUPPORT").unwrap()["sessionId"], "s1");
        let error = pick_bot(&bots, "Release").unwrap_err().to_string();
        assert!(error.contains("s2, s3"), "{error}");
        assert!(pick_bot(&bots, "Marketing").is_err());
    }

    #[test]
    fn saving_says_what_happened() {
        assert_eq!(instructions_saved("Support", ""), "Cleared the standing instructions of Support.");
        assert_eq!(
            instructions_saved("Support", "Be kind."),
            "Saved the standing instructions of Support (8 characters)."
        );
    }

    #[test]
    fn the_body_carries_the_runtime_the_way_exec_does() {
        let runtime = json!({ "id": "rt1", "provider": "codex", "label": "Codex" });
        let body = create_body("  Scout ", Some("07-ada"), Some(&runtime)).unwrap();
        assert_eq!(body, json!({ "title": "Scout", "avatar": "07-ada", "runtimeId": "rt1", "agentRuntime": "codex" }));
        assert_eq!(create_body("Scout", None, None).unwrap(), json!({ "title": "Scout" }));
    }

    #[test]
    fn a_name_the_backend_would_refuse_is_caught_here() {
        assert!(create_body("   ", None, None).is_err());
        assert!(create_body(&"x".repeat(MAX_TITLE_UNITS + 1), None, None).is_err());
        assert!(create_body(&"x".repeat(MAX_TITLE_UNITS), None, None).is_ok());
        // One emoji is two UTF-16 units: 61 of them are over the limit, 60 are not.
        assert!(create_body(&"\u{1F600}".repeat(60), None, None).is_ok());
        assert!(create_body(&"\u{1F600}".repeat(61), None, None).is_err());
    }

    #[test]
    fn an_avatar_must_look_like_a_catalog_name() {
        for ok in ["01-vermeer-girl", "29-amelia-earhart", "07-ada"] {
            assert!(create_body("Scout", Some(ok), None).is_ok(), "{ok}");
        }
        for bad in ["", "ada", "7-ada", "07ada", "07-", "07-Ada", "07-a da", "../etc", "07-\u{e9}"] {
            assert!(create_body("Scout", Some(bad), None).is_err(), "{bad}");
        }
        assert!(create_body("Scout", Some(&format!("07-{}", "a".repeat(41))), None).is_err());
    }
}
