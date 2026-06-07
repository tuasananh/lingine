import argparse
import chess
import chess.pgn
from chess.variant import find_variant
from lichess import Lichess
import chess.polyglot
import chess.engine
import engine_wrapper
import model
import matchmaking
import json
import lichess
import logging
import logging.handlers
import multiprocessing
import logging_pool
import signal
import time
import backoff
import random
import os
import io
import re
from config import load_config
from conversation import Conversation, ChatLine
from requests.exceptions import (
    ChunkedEncodingError,
    ConnectionError,
    HTTPError,
    ReadTimeout,
)
from urllib3.exceptions import ProtocolError
from ColorLogger import enable_color_logging
from collections import defaultdict
from http.client import RemoteDisconnected

logger = logging.getLogger(__name__)


def translate_xiangqi_to_engine(move_str):
    match = re.match(r"^([a-z])([0-9]+)([a-z])([0-9]+)$", move_str)
    if not match:
        return move_str
    from_file, from_rank, to_file, to_rank = match.groups()
    from_rank_eng = str(int(from_rank) - 1)
    to_rank_eng = str(int(to_rank) - 1)
    return f"{from_file}{from_rank_eng}{to_file}{to_rank_eng}"


def translate_xiangqi_to_playstrategy(move_str):
    match = re.match(r"^([a-z])([0-9]+)([a-z])([0-9]+)$", move_str)
    if not match:
        return move_str
    from_file, from_rank, to_file, to_rank = match.groups()
    from_rank_ps = str(int(from_rank) + 1)
    to_rank_ps = str(int(to_rank) + 1)
    return f"{from_file}{from_rank_ps}{to_file}{to_rank_ps}"


class CustomMove:
    def __init__(self, uci_str, variant_name="xiangqi"):
        self.uci_str = uci_str
        self.variant_name = variant_name

    def uci(self):
        return self.uci_str

    def __str__(self):
        if self.variant_name == "xiangqi":
            return translate_xiangqi_to_playstrategy(self.uci_str)
        return self.uci_str

    def __repr__(self):
        return f"CustomMove({self.uci_str!r}, variant_name={self.variant_name!r})"


STARTING_FENS = {
    "xiangqi": "rnbakabnr/9/1c5c1/p1p1p1p1p/9/9/P1P1P1P1P/1C5C1/9/RNBAKABNR w - - 0 1",
    "shogi": "lnsgkgsnl/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL b - 1",
}


class CustomVariantBoard(chess.Board):
    def __init__(self, fen=None, chess960=False, variant_name="xiangqi"):
        self.uci_variant = variant_name or "xiangqi"  # type: ignore
        self.chess960 = chess960
        self.move_stack = []
        if fen == "startpos" or not fen:
            self._fen = STARTING_FENS.get(variant_name, "startpos")
        else:
            self._fen = fen
        self.turn = chess.WHITE

    def fen(self, shredder=False, en_passant="fen", promoted=None):  # type: ignore
        return self._fen

    def root(self):
        return CustomVariantBoard(
            fen=self._fen,
            chess960=self.chess960,
            variant_name=self.uci_variant,  # type: ignore
        )

    def push_uci(self, uci_str):  # type: ignore
        move = CustomMove(uci_str, variant_name=self.uci_variant)  # type: ignore
        self.move_stack.append(move)  # type: ignore
        self.turn = chess.BLACK if self.turn == chess.WHITE else chess.WHITE
        return move

    def parse_uci(self, uci_str):  # type: ignore
        return CustomMove(uci_str, variant_name=self.uci_variant)  # type: ignore

    def push(self, move):
        if isinstance(move, str):
            move = CustomMove(move, variant_name=self.uci_variant)  # type: ignore
        elif hasattr(move, "uci"):
            move = CustomMove(move.uci(), variant_name=self.uci_variant)  # type: ignore
        self.move_stack.append(move)  # type: ignore
        self.turn = chess.BLACK if self.turn == chess.WHITE else chess.WHITE

    def pop(self):  # type: ignore
        if self.move_stack:
            self.move_stack.pop()
            self.turn = chess.BLACK if self.turn == chess.WHITE else chess.WHITE

    def is_game_over(self, claim_draw=False):  # type: ignore
        return False

    def is_check(self):
        return False

    def is_fifty_moves(self):
        return False

    def is_repetition(self):  # type: ignore
        return False

    def copy(self, stack=True):  # type: ignore
        copied = CustomVariantBoard(
            fen=self._fen,
            chess960=self.chess960,
            variant_name=self.uci_variant,  # type: ignore
        )
        if stack:
            copied.move_stack = self.move_stack.copy()
        copied.turn = self.turn
        return copied

    def variation_san(self, moves):  # type: ignore
        return " ".join(
            move.uci() if hasattr(move, "uci") else str(move) for move in moves
        )

    @property
    def occupied(self):
        return 0

    @property
    def castling_rights(self):  # type: ignore
        return 0


__version__ = "1.2.0"

terminated = False


def signal_handler(sig, frame):
    global terminated
    logger.debug(f"Received signal {sig}. Terminating client.")
    terminated = True


signal.signal(signal.SIGINT, signal_handler)
if hasattr(signal, "SIGTERM"):
    signal.signal(signal.SIGTERM, signal_handler)


def is_final(exception):
    return isinstance(exception, HTTPError) and exception.response.status_code < 500


def upgrade_account(li):
    if li.upgrade_to_bot_account() is None:
        return False

    logger.info("Succesfully upgraded to Bot Account!")
    return True


def watch_control_stream(control_queue, li):
    logger.debug("watch_control_stream: Started control stream reader")
    while not terminated:
        try:
            response = li.get_event_stream()
            logger.debug("watch_control_stream: Opened event stream")
            lines = response.iter_lines()
            for line in lines:
                if line:
                    decoded = line.decode("utf-8")
                    logger.debug(f"watch_control_stream: Received line: {decoded}")
                    event = json.loads(decoded)
                    control_queue.put_nowait(event)
                else:
                    logger.debug("watch_control_stream: Received empty line (ping)")
                    control_queue.put_nowait({"type": "ping"})
        except (BrokenPipeError, ConnectionResetError, EOFError):
            logger.info("watch_control_stream: Control queue is broken (parent process likely exited). Exiting stream reader.")
            break
        except Exception as e:
            err_msg = str(e).lower()
            if "broken pipe" in err_msg or "connection reset" in err_msg or "eof" in err_msg:
                logger.info("watch_control_stream: Control queue is broken. Exiting stream reader.")
                break
            logger.exception("watch_control_stream: Exception in stream reader loop")
            time.sleep(1)


def do_correspondence_ping(control_queue, period):
    while not terminated:
        time.sleep(period)
        try:
            control_queue.put_nowait({"type": "correspondence_ping"})
        except (BrokenPipeError, ConnectionResetError, EOFError):
            logger.info("do_correspondence_ping: Control queue is broken. Exiting pinger.")
            break
        except Exception as e:
            err_msg = str(e).lower()
            if "broken pipe" in err_msg or "connection reset" in err_msg or "eof" in err_msg:
                logger.info("do_correspondence_ping: Control queue is broken. Exiting pinger.")
                break
            logger.exception("do_correspondence_ping: Exception in pinger loop")
            time.sleep(1)


def listener_configurer(level, filename):
    logging.basicConfig(
        level=level, filename=filename, format="%(asctime)-15s: %(message)s"
    )
    enable_color_logging(level)


def logging_listener_proc(queue, configurer, level, log_filename):
    configurer(level, log_filename)
    logger = logging.getLogger()
    while not terminated:
        try:
            logger.handle(queue.get())
        except Exception:
            pass


def game_logging_configurer(queue, level):
    h = logging.handlers.QueueHandler(queue)
    root = logging.getLogger()
    root.handlers.clear()
    root.addHandler(h)
    root.setLevel(level)


def start(
    li: Lichess,
    user_profile,
    config,
    logging_level,
    log_filename,
    one_game=False,
):
    challenge_config = config["challenge"]
    max_games = challenge_config.get("concurrency", 1)
    logger.info(f"You're now connected to {config['url']} and awaiting challenges.")
    manager = multiprocessing.Manager()
    challenge_queue = manager.list()
    control_queue = manager.Queue()
    control_stream = multiprocessing.Process(
        target=watch_control_stream, args=[control_queue, li]
    )
    control_stream.daemon = True
    control_stream.start()
    correspondence_cfg = config.get("correspondence") or {}
    correspondence_checkin_period = correspondence_cfg.get("checkin_period", 600)
    correspondence_pinger = multiprocessing.Process(
        target=do_correspondence_ping,
        args=[control_queue, correspondence_checkin_period],
    )
    correspondence_pinger.daemon = True
    correspondence_pinger.start()
    correspondence_queue = manager.Queue()
    correspondence_queue.put("")
    startup_correspondence_games = [
        game["gameId"]  # type: ignore
        for game in li.get_ongoing_games()
        if game["perf"] == "correspondence"  # type: ignore
    ]  # type: ignore
    wait_for_correspondence_ping = False
    matchmaker = matchmaking.Matchmaking(li, config, user_profile["username"])

    busy_processes = 0
    queued_processes = 0

    logging_queue = manager.Queue()
    logging_listener = multiprocessing.Process(
        target=logging_listener_proc,
        args=(logging_queue, listener_configurer, logging_level, log_filename),
    )
    logging_listener.daemon = True
    logging_listener.start()

    with logging_pool.LoggingPool(max_games + 1) as pool:
        logger.info("Main event loop started")
        while not terminated:
            try:
                event = control_queue.get()
                if event.get("type") == "ping":
                    logger.debug("Received event: ping")
                else:
                    logger.debug(f"Received event: {event}")
            except InterruptedError:
                continue

            if event.get("type") is None:
                logger.warning("Unable to handle response from lichess.org:")
                logger.warning(event)
                if event.get("error") == "Missing scope":
                    logger.warning(
                        'Please check that the API access token for your bot has the scope "Play games with the bot API".'
                    )
                continue

            if event["type"] == "terminated":
                logger.info("Received terminated event, exiting main event loop")
                break
            elif event["type"] == "local_game_done":
                busy_processes -= 1
                matchmaker.last_game_ended = time.time()
                logger.info(
                    f"+++ Process Free. Total Queued: {queued_processes}. Total Used: {busy_processes}"
                )
                if one_game:
                    logger.info("one_game flag set, exiting main event loop")
                    break
            elif event["type"] == "challenge":
                chlng = model.Challenge(event["challenge"])
                logger.debug(f"Received challenge: {chlng}")
                if chlng.is_supported(challenge_config):
                    challenge_queue.append(chlng)
                    if challenge_config.get("sort_by", "best") == "best":
                        list_c = list(challenge_queue)
                        list_c.sort(key=lambda c: -c.score())
                        challenge_queue = list_c
                elif chlng.id != matchmaker.challenge_id:
                    try:
                        reason = "generic"
                        challenge = config["challenge"]
                        if not chlng.is_supported_variant(challenge["variants"]):
                            reason = "variant"
                        if not chlng.is_supported_time_control(
                            challenge["time_controls"],
                            challenge.get("max_increment", 180),
                            challenge.get("min_increment", 0),
                            challenge.get("max_base", 315360000),
                            challenge.get("min_base", 0),
                        ):
                            reason = "timeControl"
                        if not chlng.is_supported_mode(challenge["modes"]):
                            reason = "casual" if chlng.rated else "rated"
                        if (
                            not challenge.get("accept_bot", False)
                            and chlng.challenger_is_bot
                        ):
                            reason = "noBot"
                        if (
                            challenge.get("only_bot", False)
                            and not chlng.challenger_is_bot
                        ):
                            reason = "onlyBot"
                        logger.info(f"Decline {chlng} for reason '{reason}'")
                        li.decline_challenge(chlng.id, reason=reason)
                    except Exception:
                        logger.exception(f"Failed to decline challenge {chlng.id}")
                        pass
            elif event["type"] == "gameStart":
                game_id = event["game"]["id"]
                logger.info(f"Game start event for game {game_id}")
                if matchmaker.challenge_id == game_id:
                    matchmaker.challenge_id = None
                if game_id in startup_correspondence_games:
                    logger.info(f"--- Enqueue {config['url'] + game_id}")
                    correspondence_queue.put(game_id)
                    startup_correspondence_games.remove(game_id)
                else:
                    if queued_processes > 0:
                        queued_processes -= 1
                    busy_processes += 1
                    logger.info(
                        f"--- Process Used. Total Queued: {queued_processes}. Total Used: {busy_processes}"
                    )
                    logger.debug(f"Spawning play_game process for game {game_id}")
                    pool.apply_async(
                        play_game,
                        [
                            li,
                            game_id,
                            control_queue,
                            user_profile,
                            config,
                            challenge_queue,
                            correspondence_queue,
                            logging_queue,
                            game_logging_configurer,
                            logging_level,
                        ],
                    )

            is_correspondence_ping = event["type"] == "correspondence_ping"
            is_local_game_done = event["type"] == "local_game_done"
            if (
                is_correspondence_ping
                or (is_local_game_done and not wait_for_correspondence_ping)
            ) and not challenge_queue:
                if is_correspondence_ping and wait_for_correspondence_ping:
                    correspondence_queue.put("")

                wait_for_correspondence_ping = False
                while (busy_processes + queued_processes) < max_games:
                    game_id = correspondence_queue.get()
                    # stop checking in on games if we have checked in on all games since the last correspondence_ping
                    if not game_id:
                        if is_correspondence_ping and not correspondence_queue.empty():
                            correspondence_queue.put("")
                        else:
                            wait_for_correspondence_ping = True
                            break
                    else:
                        busy_processes += 1
                        logger.info(
                            f"--- Process Used. Total Queued: {queued_processes}. Total Used: {busy_processes}"
                        )
                        logger.debug(
                            f"Spawning play_game process for correspondence game {game_id}"
                        )
                        pool.apply_async(
                            play_game,
                            [
                                li,
                                game_id,
                                control_queue,
                                user_profile,
                                config,
                                challenge_queue,
                                correspondence_queue,
                                logging_queue,
                                game_logging_configurer,
                                logging_level,
                            ],
                        )

            while (
                (queued_processes + busy_processes) < max_games and challenge_queue
            ):  # keep processing the queue until empty or max_games is reached
                chlng = challenge_queue.pop(0)
                try:
                    logger.info(f"Accept {chlng}")
                    queued_processes += 1
                    li.accept_challenge(chlng.id)
                    logger.info(
                        f"--- Process Queue. Total Queued: {queued_processes}. Total Used: {busy_processes}"
                    )
                except (HTTPError, ReadTimeout) as exception:
                    if (
                        isinstance(exception, HTTPError)
                        and exception.response.status_code == 404
                    ):  # ignore missing challenge
                        logger.info(f"Skip missing {chlng}")
                    queued_processes -= 1

            if (
                queued_processes + busy_processes < min(max_games, 1)
                and not challenge_queue
                and matchmaker.should_create_challenge()
            ):
                logger.info("Challenging a random bot")
                matchmaker.challenge()

            control_queue.task_done()

    logger.info("Terminated")
    control_stream.terminate()
    control_stream.join()
    correspondence_pinger.terminate()
    correspondence_pinger.join()
    logging_listener.terminate()
    logging_listener.join()


@backoff.on_exception(backoff.expo, Exception, max_time=600, giveup=is_final)
def play_game(
    li,
    game_id,
    control_queue,
    user_profile,
    config,
    challenge_queue,
    correspondence_queue,
    logging_queue,
    logging_configurer,
    logging_level,
):
    logging_configurer(logging_queue, logging_level)
    logger = logging.getLogger(__name__)
    logger.debug(f"play_game: Started game process for game {game_id}")

    try:
        logger.debug(f"play_game: Opening stream for game {game_id}")
        response = li.get_game_stream(game_id)
        logger.debug(
            f"play_game: Received status code {response.status_code} for game stream {game_id}"
        )
        response.raise_for_status()
        lines = response.iter_lines()

        # Initial response of stream will be the full game info. Store it
        first_line = next(lines)
        if not first_line:
            logger.error(
                f"play_game: First line of stream was empty for game {game_id}"
            )
            raise ValueError("Empty initial state from stream")
        decoded_first_line = first_line.decode("utf-8")
        logger.debug(
            f"play_game: Initial stream line for game {game_id}: {decoded_first_line[:500]}"
        )
        initial_state = json.loads(decoded_first_line)
        logger.debug(f"play_game: Parsed initial state for game {game_id}")
        game = model.Game(
            initial_state,
            user_profile["username"],
            li.baseUrl,
            config.get("abort_time", 20),
        )

        logger.debug(f"play_game: Creating engine for game {game_id}")
        engine = engine_wrapper.create_engine(config)
        logger.debug(
            f"play_game: Engine created. Fetching opponent info for game {game_id}"
        )
        engine.get_opponent_info(game)
        conversation = Conversation(game, engine, li, __version__, challenge_queue)

        logger.info(f"+++ {game}")

        is_correspondence = game.perf_name == "Correspondence"
        correspondence_cfg = config.get("correspondence") or {}
        correspondence_move_time = correspondence_cfg.get("move_time", 60) * 1000

        engine_cfg = config["engine"]
        ponder_cfg = correspondence_cfg if is_correspondence else engine_cfg
        can_ponder = ponder_cfg.get("uci_ponder", False) or ponder_cfg.get(
            "ponder", False
        )
        move_overhead = config.get("move_overhead", 1000)
        delay_seconds = config.get("rate_limiting_delay", 0) / 1000
        polyglot_cfg = engine_cfg.get("polyglot", {})
        online_moves_cfg = engine_cfg.get("online_moves", {})
        draw_or_resign_cfg = engine_cfg.get("draw_or_resign") or {}

        greeting_cfg = config.get("greeting") or {}
        keyword_map = defaultdict(str, me=game.me.name, opponent=game.opponent.name)

        def get_greeting(greeting):
            return str(greeting_cfg.get(greeting) or "").format_map(keyword_map)

        hello = get_greeting("hello")
        goodbye = get_greeting("goodbye")

        first_move = True
        correspondence_disconnect_time = 0
        board = None
        while not terminated:
            move_attempted = False
            try:
                if first_move:
                    logger.debug(
                        "play_game: Processing first move update using initial game state"
                    )
                    upd = game.state
                    first_move = False
                else:
                    logger.debug(
                        f"play_game: Waiting for next game stream chunk/line for game {game_id}"
                    )
                    binary_chunk = next(lines)
                    if binary_chunk:
                        decoded_chunk = binary_chunk.decode("utf-8")
                        logger.debug(
                            f"play_game: Received chunk: {decoded_chunk[:500]}"
                        )
                        upd = json.loads(decoded_chunk)
                    else:
                        logger.debug("play_game: Received empty line chunk (ping)")
                        upd = None

                u_type = upd["type"] if upd else "ping"
                if u_type != "ping":
                    logger.debug(
                        f"play_game: Received update type: {u_type} for game {game_id}"
                    )
                if u_type == "chatLine":
                    logger.debug(f"play_game: Processing chatLine for game {game_id}")
                    conversation.react(ChatLine(upd), game)
                elif u_type == "gameState":
                    game.state = upd
                    logger.debug(
                        f"play_game: Setting up board state for game {game_id}"
                    )
                    board = setup_board(game)
                    logger.debug(f"play_game: Board fen: {board.fen()}")
                    is_over = is_game_over(game)
                    is_my_turn = is_engine_move(game, board)
                    logger.debug(
                        f"play_game: is_game_over={is_over}, is_engine_move={is_my_turn}"
                    )
                    if not is_over and is_my_turn:
                        if len(board.move_stack) < 2:
                            logger.debug(
                                f"play_game: Sending hello greeting to opponent in game {game_id}"
                            )
                            conversation.send_message("player", hello)
                        start_time = time.perf_counter_ns()
                        fake_thinking(config, board, game)
                        print_move_number(board)
                        correspondence_disconnect_time = correspondence_cfg.get(
                            "disconnect_time", 300
                        )

                        logger.debug(
                            f"play_game: Searching book move for game {game_id}"
                        )
                        best_move = get_book_move(board, polyglot_cfg)
                        if best_move.move is None:
                            logger.debug(
                                f"play_game: Book move not found, searching online move for game {game_id}"
                            )
                            best_move = get_online_move(
                                li, board, game, online_moves_cfg, draw_or_resign_cfg
                            )

                        if best_move.move is None:
                            draw_offered = check_for_draw_offer(game)
                            logger.debug(
                                f"play_game: Online move not found, using engine for game {game_id}. draw_offered={draw_offered}"
                            )

                            if len(board.move_stack) < 2:
                                logger.debug(
                                    f"play_game: Choosing first move for game {game_id}"
                                )
                                best_move = choose_first_move(
                                    engine, board, draw_offered
                                )
                            elif is_correspondence:
                                logger.debug(
                                    f"play_game: Choosing correspondence move time for game {game_id}"
                                )
                                best_move = choose_move_time(
                                    engine,
                                    board,
                                    correspondence_move_time,
                                    can_ponder,
                                    draw_offered,
                                )
                            else:
                                logger.debug(
                                    f"play_game: Choosing search move for game {game_id}"
                                )
                                best_move = choose_move(
                                    engine,
                                    board,
                                    game,
                                    can_ponder,
                                    draw_offered,
                                    start_time,
                                    move_overhead,
                                )
                        else:
                            engine.add_null_comment()

                        logger.info(
                            f"play_game: Selected move {best_move.move} (resigned={best_move.resigned}, draw_offered={best_move.draw_offered}) for game {game_id}"
                        )
                        move_attempted = True
                        if best_move.resigned and len(board.move_stack) >= 2:
                            logger.info(f"play_game: Resigning game {game_id}")
                            li.resign(game.id)
                        else:
                            logger.info(
                                f"play_game: Making move {best_move.move} in game {game.id}"
                            )
                            li.make_move(game.id, best_move)
                        time.sleep(delay_seconds)
                    elif is_game_over(game):
                        logger.info(
                            f"play_game: Game {game_id} is over, reporting results to engine"
                        )
                        engine.report_game_result(game, board)
                        tell_user_game_result(game, board)
                        conversation.send_message("player", goodbye)
                    elif len(board.move_stack) == 0:
                        correspondence_disconnect_time = correspondence_cfg.get(
                            "disconnect_time", 300
                        )

                    wb = "w" if board.turn == chess.WHITE else "b"
                    game.ping(
                        config.get("abort_time", 20),
                        (upd[f"{wb}time"] + upd[f"{wb}inc"]) / 1000 + 60,  # type: ignore
                        correspondence_disconnect_time,
                    )
                elif u_type == "ping":
                    if (
                        is_correspondence
                        and board is not None
                        and not is_engine_move(game, board)
                        and game.should_disconnect_now()
                    ):
                        logger.info(
                            f"play_game: Disconnecting from correspondence game {game_id} by timeout"
                        )
                        break
                    elif game.should_abort_now():
                        logger.info(f"Aborting {game.url()} by lack of activity")
                        li.abort(game.id)
                        break
                    elif game.should_terminate_now():
                        logger.info(f"Terminating {game.url()} by lack of activity")
                        if game.is_abortable():
                            li.abort(game.id)
                        break
            except (
                HTTPError,
                ReadTimeout,
                RemoteDisconnected,
                ChunkedEncodingError,
                ConnectionError,
                ProtocolError,
            ) as e:
                logger.warning(
                    f"play_game: Network/HTTP exception in game loop for game {game_id}: {e}"
                )
                if move_attempted:
                    logger.debug(
                        "play_game: Move was attempted prior to network exception, continuing loop"
                    )
                    continue
                if game.id not in (
                    ongoing_game["gameId"] for ongoing_game in li.get_ongoing_games()
                ):
                    logger.info(
                        f"play_game: Game {game_id} is no longer active after network exception, exiting"
                    )
                    break
            except StopIteration:
                logger.info(f"play_game: Stream iterator terminated for game {game_id}")
                break

        logger.debug(f"play_game: Stopping engine for game {game_id}")
        engine.stop()
        engine.quit()

        try:
            if board is not None and not isinstance(board, CustomVariantBoard):
                logger.debug(f"play_game: Printing PGN record for game {game_id}")
                print_pgn_game_record(li, config, game, board, engine)
        except Exception as e:
            logger.warning(f"Error writing game record: {repr(e)}")

        if is_correspondence and board is not None and not is_game_over(game):
            logger.info(f"--- Disconnecting from {game.url()}")
            correspondence_queue.put(game_id)
        else:
            logger.info(f"--- {game.url()} Game over")

    except Exception:
        logger.exception(f"play_game: Fatal exception in play_game for game {game_id}")
        raise
    finally:
        logger.debug(f"play_game: Sending local_game_done signal for game {game_id}")
        control_queue.put_nowait({"type": "local_game_done"})


def choose_move_time(engine, board, search_time, ponder, draw_offered):
    logger.info(f"Searching for time {search_time}")
    return engine.search_for(board, search_time, ponder, draw_offered)


def choose_first_move(engine, board, draw_offered):
    # need to hardcode first movetime (10000 ms) since Lichess has 30 sec limit.
    search_time = 10000
    logger.info(f"Searching for time {search_time}")
    return engine.first_search(board, search_time, draw_offered)


def get_book_move(board, polyglot_cfg):
    no_book_move = chess.engine.PlayResult(None, None)
    if (
        not polyglot_cfg.get("enabled")
        or len(board.move_stack) > polyglot_cfg.get("max_depth", 8) * 2 - 1
    ):
        return no_book_move

    book_config = polyglot_cfg.get("book", {})

    if board.uci_variant == "chess":
        books = book_config["standard"]
    else:
        if book_config.get(board.uci_variant):
            books = book_config[board.uci_variant]
        else:
            return no_book_move

    if isinstance(books, str):
        books = [books]

    for book in books:
        with chess.polyglot.open_reader(book) as reader:
            move = None
            try:
                selection = polyglot_cfg.get("selection", "weighted_random")
                if selection == "weighted_random":
                    move = reader.weighted_choice(board).move
                elif selection == "uniform_random":
                    move = reader.choice(
                        board, minimum_weight=polyglot_cfg.get("min_weight", 1)
                    ).move
                elif selection == "best_move":
                    move = reader.find(
                        board, minimum_weight=polyglot_cfg.get("min_weight", 1)
                    ).move
            except IndexError:
                # python-chess raises "IndexError" if no entries found
                move = None

        if move is not None:
            logger.info(f"Got move {move} from book {book}")
            return chess.engine.PlayResult(move, None)

    return no_book_move


def get_chessdb_move(li, board, game, chessdb_cfg):
    wb = "w" if board.turn == chess.WHITE else "b"
    if (
        not chessdb_cfg.get("enabled", False)
        or game.state[f"{wb}time"] < chessdb_cfg.get("min_time", 20) * 1000
        or board.uci_variant != "chess"
    ):
        return None

    move = None
    quality = chessdb_cfg.get("move_quality", "good")

    try:
        if quality == "best":
            data = li.api_get(
                f"https://www.chessdb.cn/cdb.php?action=querypv&board={board.fen()}&json=1"
            )
            if data["status"] == "ok":
                depth = data["depth"]
                if depth >= chessdb_cfg.get("min_depth", 20):
                    score = data["score"]
                    move = data["pv"][0]
                    logger.info(
                        f"Got move {move} from chessdb.cn (depth: {depth}, score: {score})"
                    )

        elif quality == "good":
            data = li.api_get(
                f"https://www.chessdb.cn/cdb.php?action=querybest&board={board.fen()}&json=1"
            )
            if data["status"] == "ok":
                move = data["move"]
                logger.info(f"Got move {move} from chessdb.cn")

        elif quality == "all":
            data = li.api_get(
                f"https://www.chessdb.cn/cdb.php?action=query&board={board.fen()}&json=1"
            )
            if data["status"] == "ok":
                move = data["move"]
                logger.info(f"Got move {move} from chessdb.cn")
    except Exception:
        pass

    if chessdb_cfg.get("contribute", True):
        try:
            li.api_get(
                f"http://www.chessdb.cn/cdb.php?action=queue&board={board.fen()}&json=1"
            )
        except Exception:
            pass

    return move


def get_lichess_cloud_move(li, board, game, lichess_cloud_cfg):
    wb = "w" if board.turn == chess.WHITE else "b"
    if (
        not lichess_cloud_cfg.get("enabled", False)
        or game.state[f"{wb}time"] < lichess_cloud_cfg.get("min_time", 20) * 1000
    ):
        return None

    move = None

    quality = lichess_cloud_cfg.get("move_quality", "best")
    multipv = 1 if quality == "best" else 5
    variant = "standard" if board.uci_variant == "chess" else board.uci_variant

    try:
        data = li.api_get(
            f"https://lichess.org/api/cloud-eval?fen={board.fen()}&multiPv={multipv}&variant={variant}",
            raise_for_status=False,
        )
        if "error" not in data:
            if quality == "best":
                depth = data["depth"]
                knodes = data["knodes"]
                if depth >= lichess_cloud_cfg.get(
                    "min_depth", 20
                ) and knodes >= lichess_cloud_cfg.get("min_knodes", 0):
                    pv = data["pvs"][0]
                    move = pv["moves"].split()[0]
                    score = pv["cp"]
                    logger.info(
                        f"Got move {move} from lichess cloud analysis (depth: {depth}, score: {score}, knodes: {knodes})"
                    )
            else:
                depth = data["depth"]
                knodes = data["knodes"]
                if depth >= lichess_cloud_cfg.get(
                    "min_depth", 20
                ) and knodes >= lichess_cloud_cfg.get("min_knodes", 0):
                    best_eval = data["pvs"][0]["cp"]
                    pvs = data["pvs"]
                    max_difference = lichess_cloud_cfg.get("max_score_difference", 50)
                    if wb == "w":
                        pvs = list(
                            filter(
                                lambda pv: pv["cp"] >= best_eval - max_difference, pvs
                            )
                        )
                    else:
                        pvs = list(
                            filter(
                                lambda pv: pv["cp"] <= best_eval + max_difference, pvs
                            )
                        )
                    pv = random.choice(pvs)
                    move = pv["moves"].split()[0]
                    score = pv["cp"]
                    logger.info(
                        f"Got move {move} from lichess cloud analysis (depth: {depth}, score: {score}, knodes: {knodes})"
                    )
    except Exception:
        pass

    return move


def get_online_egtb_move(li, board, game, online_egtb_cfg):
    wb = "w" if board.turn == chess.WHITE else "b"
    pieces = chess.popcount(board.occupied)
    if (
        not online_egtb_cfg.get("enabled", False)
        or game.state[f"{wb}time"] < online_egtb_cfg.get("min_time", 20) * 1000
        or board.uci_variant not in ["chess", "antichess", "atomic"]
        and online_egtb_cfg.get("source", "lichess") == "lichess"
        or board.uci_variant != "chess"
        and online_egtb_cfg.get("source", "lichess") == "chessdb"
        or pieces > online_egtb_cfg.get("max_pieces", 7)
        or board.castling_rights
    ):
        return None, None

    quality = online_egtb_cfg.get("move_quality", "best")
    variant = "standard" if board.uci_variant == "chess" else board.uci_variant

    try:
        if online_egtb_cfg.get("source", "lichess") == "lichess":
            name_to_wld = {
                "loss": -2,
                "maybe-loss": -1,
                "blessed-loss": -1,
                "draw": 0,
                "cursed-win": 1,
                "maybe-win": 1,
                "win": 2,
            }
            max_pieces = 7 if board.uci_variant == "chess" else 6
            if pieces <= max_pieces:
                data = li.api_get(
                    f"http://tablebase.lichess.ovh/{variant}?fen={board.fen()}"
                )
                if quality == "best":
                    move = data["moves"][0]["uci"]
                    wdl = name_to_wld[data["moves"][0]["category"]] * -1
                    dtz = data["moves"][0]["dtz"] * -1
                    dtm = data["moves"][0]["dtm"]
                    if dtm:
                        dtm *= -1
                else:
                    best_wdl = name_to_wld[data["moves"][0]["category"]]
                    possible_moves = list(
                        filter(
                            lambda possible_move: (
                                name_to_wld[possible_move["category"]] == best_wdl
                            ),
                            data["moves"],
                        )
                    )
                    random_move = random.choice(possible_moves)
                    move = random_move["uci"]
                    wdl = name_to_wld[random_move["category"]] * -1
                    dtz = random_move["dtz"] * -1
                    dtm = random_move["dtm"]
                    if dtm:
                        dtm *= -1
                if wdl is not None:
                    logger.info(
                        f"Got move {move} from tablebase.lichess.ovh (wdl: {wdl}, dtz: {dtz}, dtm: {dtm})"
                    )
                    return move, wdl
        elif online_egtb_cfg.get("source", "lichess") == "chessdb":

            def score_to_wdl(score):
                if score < -20000:
                    return -2
                elif score < 0:
                    return -1
                elif score == 0:
                    return 0
                elif score <= 20000:
                    return 1
                else:
                    return 2

            if quality == "best":
                data = li.api_get(
                    f"https://www.chessdb.cn/cdb.php?action=querypv&board={board.fen()}&json=1"
                )
                if data["status"] == "ok":
                    score = data["score"]
                    move = data["pv"][0]
                    logger.info(
                        f"Got move {move} from chessdb.cn (wdl: {score_to_wdl(score)})"
                    )
                    return move, score_to_wdl(score)
            else:
                data = li.api_get(
                    f"https://www.chessdb.cn/cdb.php?action=queryall&board={board.fen()}&json=1"
                )
                if data["status"] == "ok":
                    best_wdl = score_to_wdl(data["moves"][0]["score"])
                    possible_moves = list(
                        filter(
                            lambda possible_move: (
                                score_to_wdl(possible_move["score"]) == best_wdl
                            ),
                            data["moves"],
                        )
                    )
                    random_move = random.choice(possible_moves)
                    score = random_move["score"]
                    move = random_move["uci"]
                    logger.info(
                        f"Got move {move} from chessdb.cn (wdl: {score_to_wdl(score)})"
                    )
                    return move, score_to_wdl(score)
    except Exception:
        pass

    return None, None


def get_online_move(li, board, game, online_moves_cfg, draw_or_resign_cfg):
    online_egtb_cfg = online_moves_cfg.get("online_egtb", {})
    chessdb_cfg = online_moves_cfg.get("chessdb_book", {})
    lichess_cloud_cfg = online_moves_cfg.get("lichess_cloud_analysis", {})
    offer_draw = False
    resign = False
    best_move, wdl = get_online_egtb_move(li, board, game, online_egtb_cfg)
    if best_move is None:
        best_move = get_chessdb_move(li, board, game, chessdb_cfg)
    else:
        if (
            draw_or_resign_cfg.get("offer_draw_enabled", False)
            and draw_or_resign_cfg.get("offer_draw_for_egtb_zero", True)
            and wdl == 0
        ):
            offer_draw = True
        if (
            draw_or_resign_cfg.get("resign_enabled", False)
            and draw_or_resign_cfg.get("resign_for_egtb_minus_two", True)
            and wdl == -2
        ):
            resign = True

    if best_move is None:
        best_move = get_lichess_cloud_move(li, board, game, lichess_cloud_cfg)

    if best_move:
        return chess.engine.PlayResult(
            chess.Move.from_uci(best_move),
            None,
            draw_offered=offer_draw,
            resigned=resign,
        )
    return chess.engine.PlayResult(None, None)


def choose_move(engine, board, game, ponder, draw_offered, start_time, move_overhead):
    wtime = game.state["wtime"]
    btime = game.state["btime"]
    pre_move_time = int((time.perf_counter_ns() - start_time) / 1000000)
    if board.turn == chess.WHITE:
        wtime = max(0, wtime - move_overhead - pre_move_time)
    else:
        btime = max(0, btime - move_overhead - pre_move_time)

    logger.info(f"Searching for wtime {wtime} btime {btime}")
    return engine.search_with_ponder(
        board,
        wtime,
        btime,
        game.state["winc"],
        game.state["binc"],
        ponder,
        draw_offered,
    )


def check_for_draw_offer(game):
    return game.state.get(f"{game.opponent_color[0]}draw", False)


def fake_thinking(config, board, game):
    if config.get("fake_think_time") and len(board.move_stack) > 9:
        delay = min(game.clock_initial, game.my_remaining_seconds()) * 0.015
        accel = 1 - max(0, min(100, len(board.move_stack) - 20)) / 150
        sleep = min(5, delay * accel)
        time.sleep(sleep)


def print_move_number(board):
    logger.info("")
    logger.info(f"move: {len(board.move_stack) // 2 + 1}")


def setup_board(game):
    if game.variant_name.lower() == "chess960":
        board = chess.Board(game.initial_fen, chess960=True)
    elif game.variant_name == "From Position":
        board = chess.Board(game.initial_fen)
    else:
        try:
            VariantBoard = find_variant(game.variant_name)
            board = VariantBoard()
        except ValueError:
            board = CustomVariantBoard(
                fen=game.initial_fen, variant_name=game.variant_name.lower()
            )

    if game.white_starts:
        board.turn = chess.WHITE
    else:
        board.turn = chess.BLACK

    for move in game.state["moves"].split():
        try:
            if isinstance(board, CustomVariantBoard) and board.uci_variant == "xiangqi":
                move = translate_xiangqi_to_engine(move)
            board.push_uci(move)
        except ValueError as e:
            logger.debug(f"Ignoring illegal move {move} on board {board.fen()} ({e})")

    active = game.state.get("activeplayer")
    if active == "p1":
        board.turn = chess.WHITE
    elif active == "p2":
        board.turn = chess.BLACK

    return board


def is_engine_move(game, board):
    return game.is_white == (board.turn == chess.WHITE)


def is_game_over(game):
    return game.state["status"] != "started"


def tell_user_game_result(game, board):
    winner = game.state.get("winner")
    termination = game.state.get("status")

    winning_name = game.white.name if winner == "white" else game.black.name
    losing_name = game.white.name if winner == "black" else game.black.name

    if winner is not None:
        logger.info(f"{winning_name} won!")
    elif termination == engine_wrapper.Termination.DRAW:
        logger.info("Game ended in draw.")
    else:
        logger.info("Game adjourned.")

    if termination == engine_wrapper.Termination.MATE:
        logger.info("Game won by checkmate.")
    elif termination == engine_wrapper.Termination.TIMEOUT:
        logger.info(f"{losing_name} forfeited on time.")
    elif termination == engine_wrapper.Termination.RESIGN:
        logger.info(f"{losing_name} resigned.")
    elif termination == engine_wrapper.Termination.ABORT:
        logger.info("Game aborted.")
    elif termination == engine_wrapper.Termination.DRAW:
        if board.is_fifty_moves():
            logger.info("Game drawn by 50-move rule.")
        elif board.is_repetition():
            logger.info("Game drawn by threefold repetition.")
        else:
            logger.info("Game drawn by agreement.")
    elif termination:
        logger.info(f"Game ended by {termination}")


def print_pgn_game_record(li, config, game, board, engine):
    game_directory = config.get("pgn_directory")
    if not game_directory:
        return

    try:
        os.mkdir(game_directory)
    except FileExistsError:
        pass

    game_file_name = f"{game.white.name} vs {game.black.name} - {game.id}.pgn"
    game_file_name = "".join(c for c in game_file_name if c not in '<>:"/\\|?*')
    game_path = os.path.join(game_directory, game_file_name)

    lichess_game_record = chess.pgn.read_game(io.StringIO(li.get_game_pgn(game.id)))
    if lichess_game_record is None:
        return
    try:
        # Recall previously written PGN file to retain engine evaluations.
        with open(game_path) as game_data:
            game_record = chess.pgn.read_game(game_data)
        if game_record is None:
            game_record = lichess_game_record
        else:
            game_record.headers.update(lichess_game_record.headers)
    except FileNotFoundError:
        game_record = lichess_game_record

    current_node = game_record.game()  # type: ignore
    lichess_node = lichess_game_record.game()  # type: ignore
    for index, move in enumerate(board.move_stack):  # type: ignore
        if current_node.is_end() or current_node.next().move != move:  # type: ignore
            current_node = current_node.add_main_variation(move)  # type: ignore
        else:
            current_node = current_node.next()  # type: ignore

        if not lichess_node.is_end():  # type: ignore
            lichess_node = lichess_node.next()  # type: ignore
            current_node.set_clock(lichess_node.clock())  # type: ignore
            if lichess_node.comment:  # type: ignore
                if current_node.comment:  # type: ignore
                    if current_node.comment != lichess_node.comment:  # type: ignore
                        current_node.comment = (  # type: ignore
                            f"{current_node.comment} {lichess_node.comment}"  # type: ignore
                        )
                else:
                    current_node.comment = lichess_node.comment  # type: ignore

        commentary = engine.comment_for_board_index(index)  # type: ignore
        if commentary is not None:
            pv_node = current_node.parent.add_line(commentary.get("pv", []))  # type: ignore
            pv_node.set_eval(commentary.get("score"), commentary.get("depth"))  # type: ignore

    with open(game_path, "w") as game_record_destination:
        pgn_writer = chess.pgn.FileExporter(game_record_destination)
        game_record.accept(pgn_writer)


def intro():
    return (
        r"""
    .   _/|
    .  // o\
    .  || ._)  lichess-bot %s
    .  //__\
    .  )___(   Play on Lichess with a bot
    """
        % __version__
    )


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Play on Lichess with a bot")
    parser.add_argument(
        "-u",
        action="store_true",
        help="Add this flag to upgrade your account to a bot account.",
    )
    parser.add_argument(
        "-v",
        action="store_true",
        help="Verbose output. Changes log level from INFO to DEBUG.",
    )
    parser.add_argument(
        "--config", help="Specify a configuration file (defaults to ./config.yml)"
    )
    parser.add_argument(
        "-l", "--logfile", help="Log file to append logs to.", default=None
    )
    args = parser.parse_args()

    logging_level = logging.DEBUG if args.v else logging.INFO
    logging.basicConfig(
        level=logging_level, filename=args.logfile, format="%(asctime)-15s: %(message)s"
    )
    enable_color_logging(debug_lvl=logging_level)
    logger.info(intro())
    CONFIG = load_config(args.config or "./config.yml")
    li = lichess.Lichess(CONFIG["token"], CONFIG["url"], __version__, logging_level)

    user_profile = li.get_profile()
    username = user_profile["username"]  # type: ignore
    is_bot = user_profile.get("title") == "BOT"  # type: ignore
    logger.info(f"Welcome {username}!")

    if args.u and not is_bot:
        is_bot = upgrade_account(li)

    if is_bot:
        start(li, user_profile, CONFIG, logging_level, args.logfile)
    else:
        logger.error(
            f"{username} is not a bot account. Please upgrade it to a bot account!"
        )
