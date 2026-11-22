"""Private output driver for Orca. Every record is an actual speech-server request."""
import json
import os
import time

from orca.speechserver import SpeechServer as BaseServer


class SpeechServer(BaseServer):
    @classmethod
    def get_speech_server(cls, info=None):
        return cls("recorder")

    def speak(self, text=None, acss=None):
        if text:
            with open(os.environ["ORCA_TRANSCRIPT"], "a", encoding="utf-8") as stream:
                print(json.dumps({"time": time.monotonic(), "text": text}, ensure_ascii=False), file=stream)

    def speak_character(self, character, acss=None, cap_style=None):
        self.speak(character)

    def speak_key_event(self, event, acss=None):
        self.speak(event.get_key_name())
