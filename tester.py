#!/usr/local/bin/python3

import sys
import requests
from typing import Optional


class Player:
    lobby_id: Optional[int]
    token: str
    game_id: Optional[int]

    def __init__(self, lobby_id: Optional[int], game_id: Optional[int], token: str):
        self.lobby_id = lobby_id
        self.token = token
        self.game_id = game_id


class TestRunner:
    server: str

    def __init__(self, server: str):
        self.server = server

    def join_lobby(self, min_game_size: int, max_game_size: int) -> Player:
        resp = requests.post(f"http://{self.server}/lobby",
                             json={"min_game_size": min_game_size,
                                   "max_game_size": max_game_size},
                             allow_redirects=False)

        if resp.status_code == 201:
            response_json = resp.json()
            if "player_id_in_lobby" in response_json:
                return Player(lobby_id=response_json["player_id_in_lobby"], game_id=None, token=response_json["token"])
            elif "player_id_in_game" in response_json:
                return Player(lobby_id=None, game_id=response_json["player_id_in_game"], token=response_json["token"])

        raise(TestRunner.to_error("join lobby", resp))

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
                if "player_id_in_game" in response_json:
                    player.game_id = response_json["player_id_in_game"]
                    return True

        raise(TestRunner.to_error("check for game", resp))

    @classmethod
    def to_error(cls, operation: str, resp: requests.Response) -> Exception:
        return Exception(f"failed to {operation}: unexpected response: {resp.status_code}: {resp.text}")


def join_2_players_and_make_single_move(tester: TestRunner):
    player1 = tester.join_lobby(2, 4)
    player2 = tester.join_lobby(2, 4)
    tester.check_for_game(player1)
    if player1.game_id is None:
        raise(Exception("player 1 did not have after the second player joined"))
    print(f"player1 in game {player1.game_id}")
    print(f"player2 in game {player2.game_id}")


if __name__ == '__main__':
    server = sys.argv[1]
    print("running against server", server)
    tester = TestRunner(server)
    join_2_players_and_make_single_move(tester)
