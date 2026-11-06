"""Standard-library-only JSON-lines/native adapters. One client per serial request loop."""
import ctypes
import json
import subprocess
import threading

MAX_REPLY = 16 * 1024 * 1024

def _decode(text, ident):
    reply = json.loads(text)
    if reply.get('version') != 1 or reply.get('id') != ident:
        raise RuntimeError('invalid gamectx protocol reply')
    if not reply.get('ok'):
        raise ValueError(reply.get('error'))
    return reply['result']

class Client:
    def __init__(self, project, binary='gamectx'):
        self._process = subprocess.Popen([str(binary), 'serve', str(project)], stdin=subprocess.PIPE,
                                         stdout=subprocess.PIPE, text=True, encoding='utf-8')
        self._lock = threading.Lock()
        self._next = 0

    def request(self, op, args):
        with self._lock:
            self._next += 1
            ident = str(self._next)
            body = json.dumps({'version': 1, 'id': ident, 'op': op, 'args': args}, ensure_ascii=False, separators=(',', ':'), allow_nan=False)
            if len(body.encode('utf-8')) > 1572864:
                raise ValueError('gamectx request exceeds limit')
            self._process.stdin.write(body + '\n')
            self._process.stdin.flush()
            line = self._process.stdout.readline(MAX_REPLY + 1)
            if not line or len(line.encode('utf-8')) > MAX_REPLY or not line.endswith('\n'):
                self.close()
                raise RuntimeError('gamectx bridge ended or returned an oversized reply')
            return _decode(line, ident)

    def close(self):
        if self._process.poll() is None:
            self._process.stdin.close()
            try:
                self._process.wait(timeout=2)
            except subprocess.TimeoutExpired:
                self._process.kill()
                self._process.wait()
        self._process.stdout.close()

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()

class NativeClient:
    def __init__(self, project_json, library):
        self._lib = ctypes.CDLL(str(library))
        self._lib.gamectx_project_new.argtypes = [ctypes.c_char_p, ctypes.c_size_t]
        self._lib.gamectx_project_new.restype = ctypes.c_void_p
        self._lib.gamectx_project_request.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_size_t]
        self._lib.gamectx_project_request.restype = ctypes.c_void_p
        self._lib.gamectx_last_error.restype = ctypes.c_void_p
        self._lib.gamectx_string_free.argtypes = [ctypes.c_void_p]
        self._lib.gamectx_string_free.restype = None
        self._lib.gamectx_project_free.argtypes = [ctypes.c_void_p]
        self._lib.gamectx_project_free.restype = None
        body = project_json.encode('utf-8')
        self._handle = self._lib.gamectx_project_new(body, len(body))
        if not self._handle:
            raise ValueError(self._take(self._lib.gamectx_last_error()))
        self._lock = threading.Lock()
        self._next = 0

    def _take(self, pointer):
        if not pointer:
            raise RuntimeError('native gamectx call failed')
        try:
            return ctypes.string_at(pointer).decode('utf-8')
        finally:
            self._lib.gamectx_string_free(pointer)

    def request(self, op, args):
        with self._lock:
            if not self._handle:
                raise RuntimeError('native gamectx client is closed')
            self._next += 1
            ident = str(self._next)
            body = json.dumps({'version': 1, 'id': ident, 'op': op, 'args': args}, ensure_ascii=False, separators=(',', ':'), allow_nan=False).encode('utf-8')
            pointer = self._lib.gamectx_project_request(self._handle, body, len(body))
            if not pointer:
                raise ValueError(self._take(self._lib.gamectx_last_error()))
            return _decode(self._take(pointer), ident)

    def close(self):
        with self._lock:
            if self._handle:
                self._lib.gamectx_project_free(self._handle)
                self._handle = None

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()
