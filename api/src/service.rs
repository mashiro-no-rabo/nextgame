use std::collections::{HashMap, HashSet};

use jiff::ToSpan;
use jiff::civil::Weekday;
use crate::types::{Comment, Game, Team, TeamPageResponse};

use crate::random;

/// Build the API response from team + game data.
pub fn team_response(team: &Team, key: &str, game: Option<Game>) -> TeamPageResponse {
    TeamPageResponse {
        team_name: team.name.clone(),
        team_key: key.to_string(),
        location: team.location.clone(),
        time: team.time.clone(),
        weekly_schedule: team.weekly_schedule,
        default_squads: team.default_squads.clone(),
        players: team.players.clone(),
        game,
    }
}

/// Create a new game from the team's defaults.
pub fn make_new_game(team: &Team, description: String) -> Game {
    Game {
        description,
        players: team.players.keys().map(|k| (k.clone(), None)).collect(),
        guests: Vec::new(),
        comments: Vec::new(),
        date: team.weekly_schedule.map(|w| {
            jiff::Zoned::now()
                .date()
                .series(1.days())
                .find(|d| d.weekday() == Weekday::from_monday_one_offset(w).unwrap())
                .unwrap()
        }),
        squads: team.default_squads.clone(),
        squad_assignments: HashMap::new(),
        is_game_off: false,
    }
}

/// Check if a game is stale (date > 1 day ago) and should be auto-reset.
/// Returns true if the game should be replaced.
pub fn should_reset_game(team: &Team, game: &Game) -> bool {
    if team.weekly_schedule.is_none() {
        return false;
    }
    match game.date {
        Some(d) => (jiff::Zoned::now().date() - d).get_days() > 1,
        None => false,
    }
}

/// Ensure all team players exist in the game's player map.
/// Returns true if any were added.
pub fn populate_unregistered_players(team: &Team, game: &mut Game) -> bool {
    let tp_set: HashSet<_> = team.players.keys().cloned().collect();
    let gp_set: HashSet<_> = game.players.keys().cloned().collect();
    let new_players: HashMap<_, _> = tp_set
        .difference(&gp_set)
        .map(|pid| (pid.to_string(), None))
        .collect();
    if new_players.is_empty() {
        false
    } else {
        game.players.extend(new_players);
        true
    }
}

/// Set a player's status.
pub fn set_player_status(game: &mut Game, player_id: &str, playing: bool) {
    game.players.insert(player_id.to_string(), Some(playing));
}

/// Add a comment. Returns Err if empty.
pub fn add_comment(game: &mut Game, text: &str, author: Option<&str>) -> Result<(), &'static str> {
    if text.is_empty() {
        return Err("comment can't be empty");
    }
    game.comments.push(Comment::Full {
        text: text.to_string(),
        author: author.and_then(|a| if a.is_empty() { None } else { Some(a.to_string()) }),
    });
    Ok(())
}

/// Add guests from a comma-separated string. Returns Err if empty.
pub fn add_guests(game: &mut Game, names: &str) -> Result<(), &'static str> {
    if names.is_empty() {
        return Err("guest_name can't be empty");
    }
    names
        .trim()
        .split(',')
        .for_each(|g| game.guests.push(g.trim().to_string()));
    Ok(())
}

/// Remove a guest by index.
pub fn delete_guest(game: &mut Game, idx: usize) {
    if idx < game.guests.len() {
        game.guests.remove(idx);
    }
}

/// Update squad assignments.
pub fn save_squad_assignments(
    game: &mut Game,
    assignments: HashMap<String, String>,
) {
    game.squad_assignments = assignments;
}

/// Apply settings from a JSON body to a team.
pub fn apply_settings(team: &mut Team, body: &serde_json::Value) {
    if let Some(name) = body.get("name") {
        if let Some(n) = name.as_str() {
            let n = n.trim();
            if !n.is_empty() {
                team.name = n.to_string();
            }
        }
    }
    if let Some(loc) = body.get("location") {
        team.location = loc
            .as_str()
            .and_then(|s| if s.is_empty() { None } else { Some(s.to_string()) });
    }
    if let Some(t) = body.get("time") {
        team.time = t
            .as_str()
            .and_then(|s| if s.is_empty() { None } else { Some(s.to_string()) });
    }
    if let Some(w) = body.get("weekly_schedule") {
        team.weekly_schedule = w
            .as_i64()
            .and_then(|n| if (1..=7).contains(&n) { Some(n as i8) } else { None });
    }
}

/// Add players from a comma-separated string. Returns Err if empty.
pub fn add_players(team: &mut Team, names: &str) -> Result<(), &'static str> {
    if names.is_empty() {
        return Err("player names can't be empty");
    }
    names.trim().split(',').for_each(|n| {
        let n = n.trim();
        if !n.is_empty() {
            let pid = random::hex_string();
            team.players.insert(pid, n.to_string());
        }
    });
    Ok(())
}

/// Remove a player from the team roster.
pub fn delete_player(team: &mut Team, player_id: &str) {
    team.players.remove(player_id);
}

/// Reset the game: clear next_game, return the old game key if any.
pub fn reset_game(team: &mut Team) -> Option<String> {
    team.next_game.take()
}

/// Toggle is_game_off on a game.
pub fn toggle_game_off(game: &mut Game) {
    game.is_game_off = !game.is_game_off;
}

/// Set default squads on a team from a JSON object.
pub fn set_default_squads(team: &mut Team, squads: &serde_json::Map<String, serde_json::Value>) {
    team.default_squads = squads
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
        .collect();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_team(name: &str) -> Team {
        Team {
            name: name.to_string(),
            secret: "secret123".to_string(),
            next_game: None,
            players: HashMap::new(),
            location: None,
            time: None,
            weekly_schedule: None,
            default_squads: HashMap::new(),
        }
    }

    fn make_game() -> Game {
        Game {
            description: String::new(),
            players: HashMap::new(),
            guests: Vec::new(),
            comments: Vec::new(),
            date: None,
            squads: HashMap::new(),
            squad_assignments: HashMap::new(),
            is_game_off: false,
        }
    }

    // --- team_response ---

    #[test]
    fn team_response_no_game() {
        let mut team = make_team("FC Test");
        team.location = Some("Field A".into());
        let resp = team_response(&team, "abc123", None);
        assert_eq!(resp.team_name, "FC Test");
        assert_eq!(resp.team_key, "abc123");
        assert_eq!(resp.location, Some("Field A".into()));
        assert!(resp.game.is_none());
    }

    #[test]
    fn team_response_with_game() {
        let team = make_team("FC Test");
        let game = make_game();
        let resp = team_response(&team, "abc123", Some(game));
        assert!(resp.game.is_some());
    }

    #[test]
    fn team_response_includes_roster() {
        let mut team = make_team("FC Test");
        team.players.insert("p1".into(), "Alice".into());
        let resp = team_response(&team, "k", None);
        assert_eq!(resp.players.get("p1").unwrap(), "Alice");
    }

    // --- populate_unregistered_players ---

    #[test]
    fn populate_adds_missing_players() {
        let mut team = make_team("T");
        team.players.insert("p1".into(), "Alice".into());
        team.players.insert("p2".into(), "Bob".into());

        let mut game = make_game();
        game.players.insert("p1".into(), Some(true));

        let changed = populate_unregistered_players(&team, &mut game);
        assert!(changed);
        assert_eq!(game.players.len(), 2);
        assert_eq!(game.players.get("p2"), Some(&None));
    }

    #[test]
    fn populate_noop_when_all_present() {
        let mut team = make_team("T");
        team.players.insert("p1".into(), "Alice".into());

        let mut game = make_game();
        game.players.insert("p1".into(), Some(true));

        let changed = populate_unregistered_players(&team, &mut game);
        assert!(!changed);
    }

    #[test]
    fn populate_handles_empty_team() {
        let team = make_team("T");
        let mut game = make_game();
        let changed = populate_unregistered_players(&team, &mut game);
        assert!(!changed);
    }

    // --- set_player_status ---

    #[test]
    fn set_play_new_player() {
        let mut game = make_game();
        set_player_status(&mut game, "p1", true);
        assert_eq!(game.players.get("p1"), Some(&Some(true)));
    }

    #[test]
    fn set_not_play_overwrites() {
        let mut game = make_game();
        game.players.insert("p1".into(), Some(true));
        set_player_status(&mut game, "p1", false);
        assert_eq!(game.players.get("p1"), Some(&Some(false)));
    }

    // --- add_comment ---

    #[test]
    fn add_comment_ok() {
        let mut game = make_game();
        assert!(add_comment(&mut game, "Hello", None).is_ok());
        assert_eq!(game.comments.len(), 1);
        match &game.comments[0] {
            Comment::Full { text, author } => {
                assert_eq!(text, "Hello");
                assert!(author.is_none());
            }
            _ => panic!("expected Full variant"),
        }
    }

    #[test]
    fn add_comment_with_author() {
        let mut game = make_game();
        assert!(add_comment(&mut game, "Hello", Some("Alice")).is_ok());
        match &game.comments[0] {
            Comment::Full { text, author } => {
                assert_eq!(text, "Hello");
                assert_eq!(author.as_deref(), Some("Alice"));
            }
            _ => panic!("expected Full variant"),
        }
    }

    #[test]
    fn add_comment_empty_rejected() {
        let mut game = make_game();
        assert!(add_comment(&mut game, "", None).is_err());
        assert!(game.comments.is_empty());
    }

    #[test]
    fn add_comment_accumulates() {
        let mut game = make_game();
        add_comment(&mut game, "First", None).unwrap();
        add_comment(&mut game, "Second", Some("Bob")).unwrap();
        assert_eq!(game.comments.len(), 2);
    }

    // --- add_guests ---

    #[test]
    fn add_single_guest() {
        let mut game = make_game();
        assert!(add_guests(&mut game, "Charlie").is_ok());
        assert_eq!(game.guests, vec!["Charlie"]);
    }

    #[test]
    fn add_comma_separated_guests() {
        let mut game = make_game();
        add_guests(&mut game, "Alice, Bob, Charlie").unwrap();
        assert_eq!(game.guests, vec!["Alice", "Bob", "Charlie"]);
    }

    #[test]
    fn add_guests_empty_rejected() {
        let mut game = make_game();
        assert!(add_guests(&mut game, "").is_err());
    }

    // --- delete_guest ---

    #[test]
    fn delete_guest_by_index() {
        let mut game = make_game();
        game.guests = vec!["A".into(), "B".into(), "C".into()];
        delete_guest(&mut game, 1);
        assert_eq!(game.guests, vec!["A", "C"]);
    }

    #[test]
    fn delete_guest_out_of_bounds_noop() {
        let mut game = make_game();
        game.guests = vec!["A".into()];
        delete_guest(&mut game, 5);
        assert_eq!(game.guests.len(), 1);
    }

    // --- save_squad_assignments ---

    #[test]
    fn save_squads_replaces_all() {
        let mut game = make_game();
        game.squad_assignments.insert("p1".into(), "old".into());

        let mut new = HashMap::new();
        new.insert("p2".into(), "squad1".into());
        save_squad_assignments(&mut game, new);

        assert!(!game.squad_assignments.contains_key("p1"));
        assert_eq!(game.squad_assignments.get("p2"), Some(&"squad1".into()));
    }

    // --- apply_settings ---

    #[test]
    fn apply_settings_all_fields() {
        let mut team = make_team("T");
        let body = serde_json::json!({
            "location": "Stadium",
            "time": "19:00",
            "weekly_schedule": 3
        });
        apply_settings(&mut team, &body);
        assert_eq!(team.location, Some("Stadium".into()));
        assert_eq!(team.time, Some("19:00".into()));
        assert_eq!(team.weekly_schedule, Some(3));
    }

    #[test]
    fn apply_settings_empty_clears() {
        let mut team = make_team("T");
        team.location = Some("Old".into());
        let body = serde_json::json!({"location": ""});
        apply_settings(&mut team, &body);
        assert_eq!(team.location, None);
    }

    #[test]
    fn apply_settings_partial_update() {
        let mut team = make_team("T");
        team.location = Some("Keep".into());
        team.time = Some("18:00".into());
        let body = serde_json::json!({"time": "20:00"});
        apply_settings(&mut team, &body);
        assert_eq!(team.location, Some("Keep".into())); // untouched
        assert_eq!(team.time, Some("20:00".into()));
    }

    #[test]
    fn apply_settings_invalid_weekly_schedule() {
        let mut team = make_team("T");
        let body = serde_json::json!({"weekly_schedule": 0});
        apply_settings(&mut team, &body);
        assert_eq!(team.weekly_schedule, None);

        let body = serde_json::json!({"weekly_schedule": 8});
        apply_settings(&mut team, &body);
        assert_eq!(team.weekly_schedule, None);
    }

    // --- add_players ---

    #[test]
    fn add_players_single() {
        let mut team = make_team("T");
        assert!(add_players(&mut team, "Alice").is_ok());
        assert_eq!(team.players.len(), 1);
        assert!(team.players.values().any(|n| n == "Alice"));
    }

    #[test]
    fn add_players_comma_separated() {
        let mut team = make_team("T");
        add_players(&mut team, "Alice, Bob, Charlie").unwrap();
        assert_eq!(team.players.len(), 3);
    }

    #[test]
    fn add_players_empty_rejected() {
        let mut team = make_team("T");
        assert!(add_players(&mut team, "").is_err());
    }

    #[test]
    fn add_players_skips_empty_segments() {
        let mut team = make_team("T");
        add_players(&mut team, "Alice,,, Bob").unwrap();
        assert_eq!(team.players.len(), 2);
    }

    // --- delete_player ---

    #[test]
    fn delete_player_removes() {
        let mut team = make_team("T");
        team.players.insert("p1".into(), "Alice".into());
        team.players.insert("p2".into(), "Bob".into());
        delete_player(&mut team, "p1");
        assert_eq!(team.players.len(), 1);
        assert!(!team.players.contains_key("p1"));
    }

    #[test]
    fn delete_player_missing_noop() {
        let mut team = make_team("T");
        team.players.insert("p1".into(), "Alice".into());
        delete_player(&mut team, "nonexistent");
        assert_eq!(team.players.len(), 1);
    }

    // --- reset_game ---

    #[test]
    fn reset_game_clears_and_returns_key() {
        let mut team = make_team("T");
        team.next_game = Some("game123".into());
        let old = reset_game(&mut team);
        assert_eq!(old, Some("game123".into()));
        assert!(team.next_game.is_none());
    }

    #[test]
    fn reset_game_none_when_no_game() {
        let mut team = make_team("T");
        let old = reset_game(&mut team);
        assert!(old.is_none());
    }

    // --- toggle_game_off ---

    #[test]
    fn toggle_game_off_flips() {
        let mut game = make_game();
        assert!(!game.is_game_off);
        toggle_game_off(&mut game);
        assert!(game.is_game_off);
        toggle_game_off(&mut game);
        assert!(!game.is_game_off);
    }

    // --- set_default_squads ---

    #[test]
    fn set_default_squads_from_json() {
        let mut team = make_team("T");
        let squads = serde_json::json!({"1": "Blue", "2": "Red"});
        set_default_squads(&mut team, squads.as_object().unwrap());
        assert_eq!(team.default_squads.len(), 2);
        assert_eq!(team.default_squads.get("1"), Some(&"Blue".into()));
    }

    // --- make_new_game ---

    #[test]
    fn make_new_game_copies_default_squads() {
        let mut team = make_team("T");
        team.default_squads.insert("s1".into(), "Alpha".into());
        let game = make_new_game(&team, "Test game".into());
        assert_eq!(game.description, "Test game");
        assert_eq!(game.squads.get("s1"), Some(&"Alpha".into()));
        assert!(game.squad_assignments.is_empty());
        assert!(!game.is_game_off);
    }

    #[test]
    fn make_new_game_includes_team_players() {
        let mut team = make_team("T");
        team.players.insert("p1".into(), "Alice".into());
        team.players.insert("p2".into(), "Bob".into());
        let game = make_new_game(&team, String::new());
        assert_eq!(game.players.len(), 2);
        assert_eq!(game.players.get("p1"), Some(&None));
        assert_eq!(game.players.get("p2"), Some(&None));
    }

    #[test]
    fn make_new_game_no_schedule_no_date() {
        let team = make_team("T");
        let game = make_new_game(&team, String::new());
        assert!(game.date.is_none());
    }

    #[test]
    fn make_new_game_with_schedule_has_date() {
        let mut team = make_team("T");
        team.weekly_schedule = Some(3); // Wednesday
        let game = make_new_game(&team, String::new());
        assert!(game.date.is_some());
        let d = game.date.unwrap();
        assert_eq!(d.weekday(), Weekday::Wednesday);
    }

    // --- should_reset_game ---

    #[test]
    fn should_reset_no_schedule() {
        let team = make_team("T");
        let game = make_game();
        assert!(!should_reset_game(&team, &game));
    }

    #[test]
    fn should_reset_no_date() {
        let mut team = make_team("T");
        team.weekly_schedule = Some(1);
        let game = make_game();
        assert!(!should_reset_game(&team, &game));
    }

    #[test]
    fn should_reset_recent_game() {
        let mut team = make_team("T");
        team.weekly_schedule = Some(1);
        let mut game = make_game();
        game.date = Some(jiff::Zoned::now().date());
        assert!(!should_reset_game(&team, &game));
    }

    #[test]
    fn should_reset_old_game() {
        let mut team = make_team("T");
        team.weekly_schedule = Some(1);
        let mut game = make_game();
        game.date = Some(jiff::Zoned::now().date().checked_sub(5.days()).unwrap());
        assert!(should_reset_game(&team, &game));
    }
}
