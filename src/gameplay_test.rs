use super::*;
use std::convert::TryFrom;

// Test that for each precondition of each action, if the precondition is not satisfied, the action fails
// with the appropriate error.

#[test]
fn precondition_scavenge() {
    test_precondition_empty_deck(SCAVENGE);
}

#[test]
fn precondition_finish_scavenge_wrong_state() {
    test_precondition_completion_wrong_state(PlayerAction::FinishScavenge { card: c("q c") });
}

#[test]
fn precondition_finish_scavenge_wrong_player() {
    test_precondition(
        1,                                              // player 1
        action_finish_scavenge("q c"),                  // trying to finish a scavenge
        |mut g| g.state = state_scavenged(0, &["q c"]), // but it's player 0's turn to finish the scavenge
        ActionError::NotPlayersTurn { player: 1 },
    );
}

#[test]
fn precondition_finish_scavenge_wrong_card() {
    test_precondition(
        0,                                              // player 0
        action_finish_scavenge("q c"), //                  trying to accept Queen of Clubs from scavenge
        |mut g| g.state = state_scavenged(0, &["k c"]), // but the scavenge only contained King of Clubs
        ActionError::CardWasNotScavenged { card: c("q c") },
    );
}

#[test]
fn precondition_trade_wrong_card_from_player() {
    test_precondition_as(
        0,     //                                                            player 0
        TRADE, //                                                            trying to trade 2 h
        |g| {
            vec_remove_item(&mut g.players[0].gathered_parts, &c("2 h")); // but does not have it
        },
        ActionError::CardIsNotWithPlayer {
            initiating_player: true,
            player: 0,
            card: c("2 h"),
        },
    );
}

#[test]
fn precondition_trade_wrong_card_from_other_player() {
    test_precondition_as(
        0,     //                                                            player 0
        TRADE, //                                                            trying to trade player 1 for 3h
        |g| {
            vec_remove_item(&mut g.players[1].gathered_parts, &c("3 h")); // but player 1 does not have it
        },
        ActionError::CardIsNotWithPlayer {
            initiating_player: false,
            player: 1,
            card: c("3 h"),
        },
    );
}

#[test]
fn precondition_trade_escaped() {
    test_precondition_as(
        0,                               // player 0
        TRADE,                           // trying to trade with player 1
        |g| g.players[1].escaped = true, // but they already escaped
        ActionError::PlayerEscaped { escaped_player: 1 },
    );
}

#[test]
fn precondition_trade_self() {
    test_precondition_as(
        0,                              // player 0
        "trade 0 offering 2 h for 2 c", // trying to trade with themselves
        |_| (),
        ActionError::SelfTargeting,
    );
}

#[test]
fn precondition_trade_nonexistent() {
    test_precondition_as(
        0,                              // player 0
        "trade 9 offering 2 h for 4 h", // trying to trade with a non-existent player
        |_| (),
        ActionError::InvalidPlayerReference {
            non_existent_player: 9,
        },
    );
}

#[test]
fn precondition_trade_accept_wrong_state() {
    test_precondition_completion_wrong_state(PlayerAction::TradeAccept);
}

#[test]
fn precondition_trade_accept_self() {
    test_precondition(
        1,                                                   // player 1
        PlayerAction::TradeAccept,                           // trying to finish a trade
        |mut g| g.state = state_trading(1, 0, "3 h", "2 h"), // that they started themselves
        ActionError::NotPlayersTurn { player: 1 },
    );
}

#[test]
fn precondition_trade_reject_wrong_state() {
    test_precondition_completion_wrong_state(PlayerAction::TradeReject);
}

#[test]
fn precondition_trade_reject_self() {
    test_precondition(
        1,                                                   // player 1
        PlayerAction::TradeReject,                           // trying to finish a trade
        |mut g| g.state = state_trading(1, 0, "3 h", "2 h"), // that they started themselves
        ActionError::NotPlayersTurn { player: 1 },
    );
}

#[test]
fn precondition_share_empty_deck() {
    test_precondition_empty_deck(SHARE);
}

#[test]
fn precondition_share_escaped() {
    test_precondition_as(
        0,                               // player 0
        SHARE,                           // trying to share with player 1
        |g| g.players[1].escaped = true, // but they already escaped
        ActionError::PlayerEscaped { escaped_player: 1 },
    );
}

#[test]
fn precondition_share_self() {
    test_precondition_as(
        0,              // player 0
        "share with 0", // trying to trade with themselves
        |_| (),
        ActionError::SelfTargeting,
    );
}

#[test]
fn precondition_share_non_existent() {
    test_precondition_as(
        0,              // player 0
        "share with 9", // trying to share with a non-existent player
        |_| (),
        ActionError::InvalidPlayerReference {
            non_existent_player: 9,
        },
    );
}

#[test]
fn precondition_steal_wrong_card() {
    test_precondition_as(
        0,     //                                                            player 0
        STEAL, //                                                            trying to steal 3 Hearts from player 1
        |g| {
            vec_remove_item(&mut g.players[1].gathered_parts, &c("3 h")); // but player 1 does not have 3 Hearts
        },
        ActionError::CardIsNotWithPlayer {
            initiating_player: false,
            player: 1,
            card: c("3 h"),
        },
    );
}

#[test]
fn precondition_steal_escaped() {
    test_precondition_as(
        0,                               // player 0
        STEAL,                           // trying to steal from player 1
        |g| g.players[1].escaped = true, // but they already escaped
        ActionError::PlayerEscaped { escaped_player: 1 },
    );
}

#[test]
fn precondition_steal_self() {
    test_precondition_as(
        0,                  // player 0
        "steal 2 h from 0", // trying to steal from themselves
        |_| (),
        ActionError::SelfTargeting,
    );
}

#[test]
fn precondition_steal_non_existent() {
    test_precondition_as(
        0,                  // player 0
        "share 9 d from 9", // trying to steal from a non-existent player
        |_| (),
        ActionError::InvalidPlayerReference {
            non_existent_player: 9,
        },
    );
}

#[test]
fn precondition_scrap_wrong_number_of_cards() {
    test_precondition_as(
        0,                             //        player 0
        "scrap 2 h, 2 c, 2 d for q c", //        trying to scrap 3 cards
        |_| (),
        ActionError::WrongNumberOfCardsToScrap {
            //                                   but 4 is needed
            num_specified: 3,
            num_needed: 4,
        },
    );
}

#[test]
fn precondition_scrap_not_enough_unique_cards() {
    test_precondition_as(
        0,                                  //   player 0
        "scrap 2 h, 2 h, 2 c, 2 d for q c", //   trying to scrap 4 cards, out of which only 3 are unique
        |_| (),
        ActionError::WrongNumberOfCardsToScrap {
            //                                   but 4 unique ones are needed
            num_specified: 3,
            num_needed: 4,
        },
    );
}

#[test]
fn precondition_scrap_wrong_cards_from_player() {
    test_precondition_as(
        0,                                  // player 0
        "scrap 2 h, 2 c, 2 d, k d for q c", // trying to scrap some cards, including King of Diamonds
        |g| {
            //                                 but player 0 does not have Kind of Diamonds
            vec_remove_item(&mut g.players[0].gathered_parts, &c("k d"));
        },
        ActionError::CardIsNotWithPlayer {
            initiating_player: true,
            player: 0,
            card: c("k d"),
        },
    );
}

#[test]
fn precondition_scrap_wrong_card_in_scrap() {
    test_precondition_as(
        0,     //                                          player 0
        SCRAP, //                                          trying to scrap for Queen of Clubs
        |g| {
            vec_remove_item(&mut g.discard, &c("q c")); // but scrap does not have Queen of Clubs
        },
        ActionError::CardIsNotInDiscard { card: c("q c") },
    );
}

#[test]
fn precondition_escape() {
    test_precondition_as(0, ESCAPE, |_| (), ActionError::EscapeConditionNotSatisfied);
}

// Tests that for each action when it is successfully executed, the game state transitions to the correct state,
// and any side effects by the action are correctly effected.

#[test]
fn transition_scavenge() {
    // Scavenge
    let game_after_scavenge = test_state_transition_as(
        0,                                                 //      player 0
        SCAVENGE,                                          //      starts a scavenge
        |g| g.draw = Deck::of(cs(&["5 d", "6 d", "7 d"])), //      the scavenge unearths these cards
        state_scavenged(0, &["5 d", "6 d", "7 d"]),
    );

    // the player does not get any cards yet
    assert_player_does_not_have_cards(&game_after_scavenge, 0, &["5 d", "6 d", "7 d"]);

    // FinishScavenge
    let game_after_scavenge = test_state_transition_from(
        0,                             //                          then the same player
        action_finish_scavenge("5 d"), //                          finishes the action by choosing 5 d from the loot
        game_after_scavenge,
        GameState::WaitingForPlayerAction { player: 1 }, //        which ends player 0's turn
    );
    assert_player_has_cards(&game_after_scavenge, 0, &["5 d"]); // after this player 0 has the card they chose
    assert_discard_has_cards(&game_after_scavenge, &["6 d", "7 d"]);
}

#[test]
fn transition_scavenge_1card() {
    // same with just 1 card in the draw
    let game_after_scavenge = test_state_transition_as(
        0,        //                                               player 0
        SCAVENGE, //                                               starts a scavenge
        |g| {
            g.discard = Vec::new(); //                             the discard is empty
            g.draw = Deck::of(cs(&["5 d"])); //                    and the scavenge unearths just one card
        },
        state_scavenged(0, &["5 d"]),
    );
    let game_after_scavenge = test_state_transition_from(
        0,                             //                          then the same player
        action_finish_scavenge("5 d"), //                          finishes the action by choosing that one card
        game_after_scavenge,
        GameState::WaitingForPlayerAction { player: 1 }, //        which ends player 0's turn
    );
    assert_player_has_cards(&game_after_scavenge, 0, &["5 d"]); // after this player 0 has the card they chose
    assert!(
        &game_after_scavenge.discard.is_empty(), //                and the discard is still empty
        "the discard has cards somehow"
    );
}

#[test]
fn transition_share() {
    // Share
    let game_after_share = test_state_transition_as(
        0,     //                                                      player 0
        SHARE, //                                                      starts a share with player 1
        |g| g.draw = Deck::of(cs(&["5 d", "6 d", "7 d"])), //          unearthing these cards
        GameState::WaitingForPlayerAction { player: 1 }, //            which ends player 0's turn
    );
    assert_player_has_cards(&game_after_share, 0, &["5 d", "6 d"]); // after this player 0 has the first 2 cards
    assert_player_has_cards(&game_after_share, 1, &["7 d"]); //        and player 1 has the 3rd
}

#[test]
fn transition_share_2cards() {
    // same with just 2 cards in the draw
    let game_after_share = test_state_transition_as(
        0,     //                                                      player 0
        SHARE, //                                                      starts a share with player 1
        |g| {
            g.players[1].gathered_parts = Vec::new(); //               who had nothing to begin with
            g.draw = Deck::of(cs(&["5 d", "6 d"])); //                 the share unearts just 2 cards
        },
        GameState::WaitingForPlayerAction { player: 1 },
    );
    assert_player_has_cards(&game_after_share, 0, &["5 d", "6 d"]); // which player 0 gets
    assert!(
        game_after_share.players[1].gathered_parts.is_empty(), //      player 1 got nothing because there weren't
        //                                                             enough cards to begin with
        "player 1 got some parts somehow",
    );
}

#[test]
fn transition_trade_accept() {
    // Trade + TradeAccept
    let game_after_trade = test_state_transition_as(
        0,     //                                               player 0
        TRADE, //                                               starts a trade with player 1 offering 2 h for 3 h
        |_| (),
        state_trading(0, 1, "2 h", "3 h"), //                   after which the game is waiting for 1 to confirm
    );
    assert_player_has_cards(&game_after_trade, 0, &["2 h"]); // no trade had taken place yet for player 0
    assert_player_does_not_have_cards(&game_after_trade, 1, &["2 h"]);
    assert_player_has_cards(&game_after_trade, 1, &["3 h"]); // or player 1
    assert_player_does_not_have_cards(&game_after_trade, 0, &["3 h"]);

    let game_after_trade = test_state_transition_from(
        1,                         //                           player 1
        PlayerAction::TradeAccept, //                           accepts the trade
        game_after_trade,
        GameState::WaitingForPlayerAction { player: 1 }, //     which ends player 0's turn
    );
    assert_player_has_cards(&game_after_trade, 1, &["2 h"]); // and the trade takes place: player 1 gets 2 h
    assert_player_has_cards(&game_after_trade, 0, &["3 h"]); // and player 0 gets 3 h
}

#[test]
fn transition_trade_reject() {
    // Trade + TradeReject
    let game_after_trade = test_state_transition_as(
        0,     //                                               player 0
        TRADE, //                                               starts a trade with player 1 offering 2 h for 3 h
        |_| (),
        state_trading(0, 1, "2 h", "3 h"), //                   after which the game is waiting for 1 to confirm
    );

    let game_after_trade = test_state_transition_from(
        1,                         //                           player 1
        PlayerAction::TradeReject, //                           rejects the trade
        game_after_trade,
        GameState::WaitingForPlayerAction { player: 0 }, //     so it's player 0's turn again
    );
    assert_player_has_cards(&game_after_trade, 0, &["2 h"]); // no trade had taken place for player 0
    assert_player_does_not_have_cards(&game_after_trade, 1, &["2 h"]);
    assert_player_has_cards(&game_after_trade, 1, &["3 h"]); // or player 1
    assert_player_does_not_have_cards(&game_after_trade, 0, &["3 h"]);
}

#[test]
fn transition_steal() {
    // Steal
    let game_after_steal = test_state_transition_as(
        0,     //                                                         player 0
        STEAL, //                                                         steals 3 h from player 1
        |_| (),
        GameState::WaitingForPlayerAction { player: 1 }, //               which ends player 0's turn
    );
    assert_player_has_cards(&game_after_steal, 0, &["3 h"]); //           after that player 0 has 3 h
    assert_player_does_not_have_cards(&game_after_steal, 1, &["3 h"]); // and player one no longer has it
}

#[test]
fn transition_scrap() {
    // Scrap
    let game_after_scrap = test_state_transition_as(
        0,     // player 0
        SCRAP, // scraps 2 h, 2 c, 2 d, a d for q c
        |_| (),
        GameState::WaitingForPlayerAction { player: 1 }, // which ends their turn
    );
    assert_player_does_not_have_cards(&game_after_scrap, 0, &["2 h", "2 c", "2 d", "a d"]);
    assert_discard_has_cards(&game_after_scrap, &["2 h", "2 c", "2 d", "a d"]);
    assert_player_has_cards(&game_after_scrap, 0, &["q c"]);
    assert_discard_does_not_have_cards(&game_after_scrap, &["q c"]);
}

#[test]
fn transition_escape() {
    // Escape
    let game_after_escape = test_state_transition_as(
        0,      //                                                              player 0
        ESCAPE, //                                                              escapes
        |g| g.players[0].gathered_parts = cs(&["2 h", "2 d", "2 c", "2 s"]),
        GameState::WaitingForPlayerAction { player: 1 }, //                     which ends their turn
    );
    assert!(
        game_after_escape.players[0].escaped, //                                after that they're escaped
        "player 0 did not escape",
    );
}

#[test]
fn transition_skip() {
    // Skip
    test_state_transition_as(
        0,    //                                            player 0
        SKIP, //                                            skips a turn
        |_| (),
        GameState::WaitingForPlayerAction { player: 1 }, // which ends their turn
    );
}

#[test]
fn transition_cheat() {
    // Cheat
    let game_after_cheating = test_state_transition_as(
        0,               //                                         player 0
        CHEAT_GET_CARDS, //                                         cheats to get 10 d
        |_| (),
        GameState::WaitingForPlayerAction { player: 0 }, //         after which they get another turn to cheat more
    );
    assert_player_has_cards(&game_after_cheating, 0, &["10 d"]); // and they have 10 d
}

// Test that players who have escaped our out of move are not scheduled for a turn

#[test]
fn skip_escaped_out_of_move_players() {
    let mut game = basic_game(&vec![
        vec!["2 h", "2 c", "2 d", "2 s"], //                         player 0 has the cards to escape
        vec!["3 h", "3 c", "3 d", "a c"],
        vec!["4 h", "4 c", "4 d", "k c"],
    ]);

    game.process_player_action(0, PlayerAction::Escape).unwrap(); // and does so on their turn
    assert_player_moves(&game, 1, Some(1)); //                       the other players have 1 move left
    assert_player_moves(&game, 2, Some(1));
    game.process_player_action(1, PlayerAction::Skip).unwrap(); //   and they skip
    assert_player_moves(&game, 1, Some(0)); //                       using up their last move
    game.process_player_action(2, PlayerAction::Skip).unwrap();
    assert_player_moves(&game, 2, Some(0));
    assert_eq!(
        //                                                           which ends the game
        game.state,
        GameState::Finished,
        "the game state was not Finished, was {:?}",
        game.state
    );
}

#[test]
fn skip_escaped_out_of_move_players_autoescape() {
    // same for an escape triggered by receiving the missing card
    let mut game = basic_game(&vec![
        vec!["2 h", "2 c", "2 d", "2 s"], //                         player 0 has the cards to escape
        vec!["3 h", "3 c", "3 d", "a c"],
        vec!["4 h", "4 c", "4 d", "k c"],
    ]);
    let missing_card = game.players[0].missing_part;
    game.draw = Deck::of(vec![missing_card]);

    game.process_player_action(0, PlayerAction::Scavenge) //         they scavenge
        .unwrap();
    game.process_player_action(
        0,
        PlayerAction::FinishScavenge { card: missing_card }, //      and find their missing part
    )
    .unwrap();
    assert!(game.players[0].escaped, "player 0 was not escaped"); // which lets them to escape, triggering the end game
    assert_player_moves(&game, 1, Some(1)); //                       the other players have 1 move left
    assert_player_moves(&game, 2, Some(1));
    game.process_player_action(1, PlayerAction::Skip).unwrap(); //   and they skip
    assert_player_moves(&game, 1, Some(0)); //                       using up their last move
    game.process_player_action(2, PlayerAction::Skip).unwrap();
    assert_player_moves(&game, 2, Some(0));
    assert_eq!(
        //                                                           which ends the game
        game.state,
        GameState::Finished,
        "the game state was not Finished, was {:?}",
        game.state
    );
}

// Other tests

#[test]
fn get_results() {
    let mut game = basic_game(&vec![
        vec!["2 h", "2 c", "2 d", "2 s"], //                         player 0 has the cards to escape
        vec!["3 h", "3 c", "3 d", "a c"],
        vec!["4 h", "4 c", "4 d", "k c"],
    ]);
    let missing_card = game.players[0].missing_part;
    game.draw = Deck::of(vec![missing_card]);

    game.process_player_action(0, PlayerAction::Scavenge) //         they scavenge
        .unwrap();
    game.process_player_action(
        0,
        PlayerAction::FinishScavenge { card: missing_card }, //      and find their missing part
    )
    .unwrap();
    game.process_player_action(
        1,
        PlayerAction::CheatGetCards {
            cards: cs(&["3 s"]), //                                  player 1 cheats to satisfy the escape condition
        },
    )
    .unwrap();
    game.process_player_action(1, PlayerAction::Skip).unwrap(); //   and then passes
    game.process_player_action(2, PlayerAction::Skip).unwrap(); //   player 2 passes
    assert_eq!(
        //                                                           which ends the game
        game.state,
        GameState::Finished,
        "the game state was not Finished, was {:?}",
        game.state
    );

    let results = game.get_results();
    assert_eq!(results.winners, vec![0]);
    assert_eq!(results.escaped_but_not_winner, vec![1]);
    assert_eq!(results.stuck, vec![2]);
}

/// Tests that for each turn action, the preconditions are enforced properly:
/// - can only be called when it's the specified player's turn
/// - cannot be called in other states.
#[test]
fn test_turn_action_preconditions() {
    for action in turn_actions() {
        test_turn_action_precondition_correct_player(action);
        test_turn_action_precondition_correct_state(action);
    }
    // The following conditions:
    // - player escaped
    // - player is out of moves
    // are covered by `skip_escaped_out_of_move_players`.
}

// Helpers

fn test_turn_action_precondition_correct_player(action: &str) {
    test_precondition_as(
        0,
        action,
        |mut g| g.state = GameState::WaitingForPlayerAction { player: 1 },
        ActionError::NotPlayersTurn { player: 0 },
    )
}

fn test_turn_action_precondition_correct_state(action: &str) {
    test_precondition_as(
        0,
        action,
        |mut g| {
            g.state = GameState::WaitingForScavengeComplete {
                player: 0,
                scavenged_cards: Vec::new(),
            }
        },
        ActionError::NotPlayersTurn { player: 0 },
    )
}

fn test_precondition_empty_deck(action: &str) {
    test_precondition_as(
        0,
        action,
        |mut g| g.draw = empty_deck(),
        ActionError::DeckEmpty,
    )
}

fn test_precondition_completion_wrong_state(action: PlayerAction) {
    test_precondition(0, action, |_| (), ActionError::NotPlayersTurn { player: 0 });
}

fn test_precondition_as<F: Fn(&mut Gameplay)>(
    player: usize,
    action: &str,
    game_setup: F,
    expected_action_error: ActionError,
) {
    let action = PlayerAction::try_from(action).unwrap();
    test_precondition(player, action, game_setup, expected_action_error);
}

fn test_precondition<F: Fn(&mut Gameplay)>(
    player: usize,
    action: PlayerAction,
    game_setup: F,
    expected_action_error: ActionError,
) {
    let mut game = basic_2_player_with_cards();
    game_setup(&mut game);
    assert_eq!(
        game.process_player_action(player, action).unwrap_err(),
        expected_action_error,
    );
}

fn test_state_transition_as<F: Fn(&mut Gameplay)>(
    player: usize,
    action: &str,
    game_setup: F,
    expected_state: GameState,
) -> Gameplay {
    let action = PlayerAction::try_from(action).unwrap();
    test_state_transition(player, action, game_setup, expected_state)
}

fn test_state_transition<F: Fn(&mut Gameplay)>(
    player: usize,
    action: PlayerAction,
    game_setup: F,
    expected_state: GameState,
) -> Gameplay {
    let mut game = basic_2_player_with_cards();
    game_setup(&mut game);
    test_state_transition_from(player, action, game, expected_state)
}

fn test_state_transition_from(
    player: usize,
    action: PlayerAction,
    mut game: Gameplay,
    expected_state: GameState,
) -> Gameplay {
    game.process_player_action(player, action).unwrap();
    let new_state = game.get_state();
    assert_eq!(*new_state, expected_state);
    game
}

// turn action constants & helpers for constructions:
// these all assume the `basic_2_player_with_cards` setup, and that it's player 0's turn.
static SCAVENGE: &'static str = "scavenge";
static TRADE: &'static str = "trade 1 offering 2 h for 3 h";
static STEAL: &'static str = "steal 3 h from 1";
static SHARE: &'static str = "share with 1";
static SCRAP: &'static str = "scrap 2 h, 2 c, 2 d, a d for q c";
static ESCAPE: &'static str = "escape";
static SKIP: &'static str = "skip";
static CHEAT_GET_CARDS: &'static str = "conjure 10 d";
fn turn_actions() -> Vec<&'static str> {
    vec![
        SCAVENGE,
        TRADE,
        STEAL,
        SHARE,
        SCRAP,
        ESCAPE,
        SKIP,
        CHEAT_GET_CARDS,
    ]
}

fn action_finish_scavenge(card: &str) -> PlayerAction {
    PlayerAction::FinishScavenge { card: c(card) }
}

// primitives for constructing Gameplay objects

fn basic_2_player_with_cards() -> Gameplay {
    basic_game(&vec![
        vec!["2 h", "2 c", "2 d", "a d"],
        vec!["3 h", "3 c", "3 d", "a c"],
    ])
}

fn basic_game(card_strs_per_player: &Vec<Vec<&str>>) -> Gameplay {
    Gameplay {
        players: players_with_cards(card_strs_per_player),
        draw: Deck::shuffle(),
        discard: vec![c("q c")],
        state: GameState::WaitingForPlayerAction { player: 0 },
    }
}

fn empty_deck() -> Deck {
    let mut deck = Deck::shuffle();
    deck.remove_top(52);
    deck
}

fn players_with_cards(card_strs_per_player: &Vec<Vec<&str>>) -> Vec<Player> {
    let mut players = Vec::new();
    for card_strs in card_strs_per_player {
        let mut cards = Vec::new();
        for card_str in card_strs {
            cards.push(c(*card_str));
        }
        players.push(Player {
            missing_part: c("Ace of Spades"),
            gathered_parts: cards,
            escaped: false,
            moves_left: None,
        });
    }
    players
}

fn state_scavenged(player: usize, card_strs: &[&str]) -> GameState {
    GameState::WaitingForScavengeComplete {
        player: player,
        scavenged_cards: cs(card_strs),
    }
}

fn state_trading(
    initiating_player: usize,
    trading_with_player: usize,
    offering_str: &str,
    in_exchange_str: &str,
) -> GameState {
    GameState::WaitingForTradeConfirmation {
        initiating_player,
        trading_with_player,
        offer: TradeOffer {
            offered: c(offering_str),
            in_exchange: c(in_exchange_str),
        },
    }
}

// assertions
fn assert_player_has_cards(game: &Gameplay, player: usize, card_strs: &[&str]) {
    assert_collection_has_cards(
        &game.players[player].gathered_parts,
        &format!("player {}'s cards", player),
        card_strs,
    );
}

fn assert_discard_has_cards(game: &Gameplay, card_strs: &[&str]) {
    assert_collection_has_cards(&game.discard, "the discard pile", card_strs);
}

fn assert_collection_has_cards(collection: &Vec<Card>, collection_name: &str, card_strs: &[&str]) {
    let cards = cs(card_strs);
    for card in cards {
        assert!(
            collection.contains(&card),
            "{} did not contain all of {} ({} was missing), they were: {}",
            collection_name,
            cs(card_strs)
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<String>>()
                .join(", "),
            card,
            collection
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<String>>()
                .join(", "),
        );
    }
}

fn assert_player_does_not_have_cards(game: &Gameplay, player: usize, card_strs: &[&str]) {
    assert_collection_does_not_have_cards(
        &game.players[player].gathered_parts,
        &format!("player {}'s cards", player),
        card_strs,
    );
}

fn assert_discard_does_not_have_cards(game: &Gameplay, card_strs: &[&str]) {
    assert_collection_does_not_have_cards(&game.discard, "the discard pile", card_strs);
}

fn assert_collection_does_not_have_cards(
    collection: &Vec<Card>,
    collection_name: &str,
    card_strs: &[&str],
) {
    let cards = cs(card_strs);
    for card in cards {
        assert!(
            !collection.contains(&card),
            "{} contain {} (they should not have contained any of {}). They were: {}",
            collection_name,
            card,
            cs(card_strs)
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<String>>()
                .join(", "),
            collection
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<String>>()
                .join(", "),
        );
    }
}

fn assert_player_moves(game: &Gameplay, player: usize, moves: Option<u32>) {
    let actual_moves_left: Option<u32> = game.players[player].moves_left;
    assert_eq!(
        actual_moves_left, moves,
        "player {} did not have {:?} move left, had {:?}",
        player, moves, actual_moves_left,
    );
}

fn c(card_str: &str) -> Card {
    Card::try_from(card_str).unwrap()
}

fn cs(card_strs: &[&str]) -> Vec<Card> {
    let mut cards = Vec::new();
    for card_str in card_strs {
        cards.push(c(*card_str));
    }
    cards
}
