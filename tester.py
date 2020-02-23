#!/usr/local/bin/python3

import sys
import requests
from typing import Optional
import jsonschema
import json
import copy


class SchemaValidatorHelper:

    def __init__(self, schema_file: str):
        self.schema = json.load(open(schema_file))

    def validate(self, json, type_name: str):
        schema = copy.deepcopy(self.schema)
        del schema["oneOf"]
        schema["$ref"] = f"#/definitions/{type_name}"
        jsonschema.validate(json, schema)


class PlayerGameState:

    def __init__(self, secret_card):
        self.secret_card = secret_card

    def update_game_description(self, json):
        self.game_description = json

    def get_valid_action(self):
        return "Scavenge"


class Player:
    lobby_id: Optional[int]
    token: str
    game_id: Optional[int]
    player_id: Optional[int]
    game_state: Optional[PlayerGameState]

    def __init__(self, lobby_id: Optional[int], game_id: Optional[int], player_id: Optional[int], token: str):
        self.lobby_id = lobby_id
        self.token = token
        self.game_id = game_id
        self.player_id = player_id
        self.game_state = None


class Backend:
    server: str

    def __init__(self, server: str, schema: SchemaValidatorHelper):
        self.server = server
        self.schema = schema

    def join_lobby(self, min_game_size: int, max_game_size: int) -> Player:
        resp = requests.post(f"http://{self.server}/lobby",
                             json={"min_game_size": min_game_size,
                                   "max_game_size": max_game_size},
                             allow_redirects=False)

        if resp.status_code == 201:
            response_json = resp.json()
            schema.validate(response_json, "join_lobby_response")
            if "player_id_in_lobby" in response_json:
                return Player(
                    lobby_id=response_json["player_id_in_lobby"],
                    game_id=None,
                    player_id=None,
                    token=response_json["token"])
            elif "player_id_in_game" in response_json:
                return Player(
                    lobby_id=None,
                    game_id=response_json["game_id"],
                    player_id=response_json["player_id_in_game"],
                    token=response_json["token"])

        raise(Backend.to_error("join lobby", resp))

    def check_for_game(self, player: Player) -> bool:
        if player.lobby_id is not None:
            resp = requests.get(
                f"http://{self.server}/lobby/players/{player.lobby_id}/game",
                headers={"Authorization": player.token},
                allow_redirects=False)
            if resp.status_code == 404:
                return False
            elif resp.status_code == 307:
                response_json = resp.json()
                schema.validate(response_json, "found_game_response")
                if "player_id_in_game" in response_json:
                    player.game_id = response_json["game_id"]
                    player.player_id = response_json["player_id_in_game"]
                    return True

            raise(Backend.to_error("check for game", resp))
        else:
            raise(Exception("tried to check for game on player not in lobby"))

    def check_secret_card(self, player: Player):
        if player.game_id is not None:
            resp = requests.get(
                f"http://{self.server}/games/{player.game_id}/players/{player.player_id}/private",
                headers={"Authorization": player.token})
            if resp.status_code == 200:
                response_json = resp.json()
                schema.validate(response_json, "player_private_response")
                state = PlayerGameState(
                    secret_card=response_json["missing_part"])
                player.game_state = state
                return
            raise(Backend.to_error("check for secret card", resp))
        else:
            raise(Exception("tried to check for secret card on player not in a game"))

    def make_move(self, player: Player):
        if player.game_state is not None:
            resp = requests.post(
                f"http://{self.server}/games/{player.game_id}/players/{player.player_id}/moves",
                json=player.game_state.get_valid_action(),
                headers={"Authorization": player.token})
            if resp.status_code == 200:
                self.refresh_game_state(player)
                return
            raise(Backend.to_error("make a move", resp))
        else:
            raise(Exception("tried to make move with player without a game state"))

    def refresh_game_state(self, player: Player):
        if player.game_state is not None:
            # this endpoint is public
            resp = requests.get(f"http://{self.server}/games/{player.game_id}")
            if resp.status_code == 200:
                response_json = resp.json()
                schema.validate(response_json, "game_description")
                player.game_state.update_game_description(response_json)
                return
            raise(Backend.to_error("get game state", resp))
        else:
            raise(Exception("tried to get game state for player without a game state"))

    @classmethod
    def to_error(cls, operation: str, resp: requests.Response) -> Exception:
        return Exception(f"failed to {operation}: unexpected response: {resp.status_code}: {resp.text}")


def join_2_players_and_make_single_move(backend: Backend):
    user1 = backend.join_lobby(2, 4)
    user2 = backend.join_lobby(2, 4)
    backend.check_for_game(user1)
    if user1.game_id is None:
        raise(Exception("user1 did not have after the second user joined"))

    print(f"user1 in game {user1.game_id} (player {user1.player_id})")
    print(f"user2 in game {user2.game_id} (player {user2.player_id})")
    backend.check_secret_card(user1)
    if user1.game_state is None:
        raise(Exception("user1 did not have a secret card after checking"))
    print(f"user1 secret card: {user1.game_state.secret_card}")
    backend.check_secret_card(user2)
    if user2.game_state is None:
        raise(Exception("user2 did not have a secret card after checking"))
    print(f"user2 secret card: {user2.game_state.secret_card}")
    backend.refresh_game_state(user1)
    backend.make_move(user1)
    print(f"game state for user2: {user1.game_state.game_description}")


if __name__ == '__main__':
    server = sys.argv[1]
    schema_file = sys.argv[2]
    print("running against server", server)
    print("validating against schema", schema_file)
    schema = SchemaValidatorHelper(schema_file)
    backend = Backend(server, schema)
    join_2_players_and_make_single_move(backend)
