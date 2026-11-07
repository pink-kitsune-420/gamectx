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
    def __init__(self, project=None, binary='gamectx'):
        self._process = subprocess.Popen([str(binary), *(['characters'] if project is None else ['serve', str(project)])], stdin=subprocess.PIPE,
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
        prefix='gamectx_session' if hasattr(self._lib,'gamectx_session_new') else 'gamectx_project'
        self._new=getattr(self._lib,prefix+'_new');self._request=getattr(self._lib,prefix+'_request');self._free=getattr(self._lib,prefix+'_free')
        self._new.argtypes = [ctypes.c_char_p, ctypes.c_size_t]
        self._new.restype = ctypes.c_void_p
        self._request.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_size_t]
        self._request.restype = ctypes.c_void_p
        self._lib.gamectx_last_error.restype = ctypes.c_void_p
        self._lib.gamectx_string_free.argtypes = [ctypes.c_void_p]
        self._lib.gamectx_string_free.restype = None
        self._free.argtypes = [ctypes.c_void_p]
        self._free.restype = None
        if project_json is None:project_json='{"version":1,"datasets":[],"tools":[],"recipes":[]}'
        body = project_json.encode('utf-8')
        self._handle = self._new(body, len(body))
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
            pointer = self._request(self._handle, body, len(body))
            if not pointer:
                raise ValueError(self._take(self._lib.gamectx_last_error()))
            return _decode(self._take(pointer), ident)

    def close(self):
        with self._lock:
            if self._handle:
                self._free(self._handle)
                self._handle = None

    def __enter__(self):
        return self

    def __exit__(self, *_):
        self.close()

class Character:
    """One host-chosen actor/audience scope on a persistent client.

    Bind a fresh redacted view before each turn. Pass only model function arguments
    to call(); actor, scope, current location and permissions stay in the host.
    """
    def __init__(self, client, ident, scope, actor, participants):
        self.client, self.ident = client, ident
        self._request('create', scope=scope, actor=actor, participants=participants)

    def _request(self, operation, **args):
        return self.client.request('character_' + operation, {**args, 'id': self.ident})

    def bind(self, view):
        return self._request('bind', view=view)

    def enable(self, tools):
        return self._request('enable', tools=tools)

    def call(self, name, arguments, revision, max_bytes=2048, allowed=()):
        text = json.dumps(arguments, ensure_ascii=False, separators=(',', ':'), allow_nan=False)
        return self._request('call', name=name, text=text, revision=revision,
                             max_bytes=max_bytes, allowed=list(allowed))

    def prepare(self, plan_id, identity, budget, turn):
        return self._request('prepare', plan_id=plan_id, identity=identity, budget=budget, turn=turn)

    def observe(self, observation):
        return self._request('observe', observation=observation)

    def routine_configure(self, calendar, definition=None, defaults=None, overrides=None):
        return self._request('routine_configure', calendar=calendar, definition=definition,
                             defaults={} if defaults is None else defaults, overrides={} if overrides is None else overrides)

    def routine_update(self, frame):
        return self._request('routine_update', frame=frame)

    def routine_observation(self, observation):
        return self._request('routine_observation', observation=observation)

    def routine_complete(self, activity, start, revision, token):
        return self._request('routine_complete', activity=activity, start=start, revision=revision, token=token)

    def routine_reserve(self, revision):
        return self._request('routine_reserve', revision=revision)

    def routine_context(self, selection, revision, max_bytes=8192):
        return self._request('routine_context', selection=selection, revision=revision, max_bytes=max_bytes)

    def routine_assess_time(self, revision, max_bytes=8192):
        return self._request('routine_assess_time', revision=revision, max_bytes=max_bytes)

    def routine_wake(self):
        return self._request('routine_wake')

    def routine_affected(self, conditions=(), activities=()):
        return self._request('routine_affected', conditions=list(conditions), activities=list(activities))

    def routine_save(self):
        return self._request('routine_save')

    def routine_restore(self, state, min_revision, min_observation, min_completion, min_tick):
        return self._request('routine_restore', state=state, min_revision=min_revision,
                             min_observation=min_observation, min_completion=min_completion, min_tick=min_tick)

    def life(self, operation, **args):
        """Privileged life host operation; never expose this method as a model tool."""
        return self._request('life_' + operation, **args)

    def life_configure(self, setting, definition, defaults=None, overrides=None):
        return self.life('configure', setting=setting, definition=definition,
                         defaults={} if defaults is None else defaults,
                         overrides={} if overrides is None else overrides)

    def life_experience(self, event):
        return self.life('experience', event=event)

    def life_appraise(self, rule, event, revision):
        return self.life('appraise', rule=rule, event=event, revision=revision)

    def life_context(self, tags, caps):
        return self.life('context', tags=tags, caps=caps)

    def life_recall(self, text, tags=(), count=2):
        return self.life('recall', input=text, tags=list(tags), count=count)

    def continuity(self, operation, **args):
        return self._request('continuity_' + operation, **args)

    def initiative(self, operation, **args):
        return self._request('initiative_' + operation, **args)

    def event(self, event):
        return self._request('event', event=event)

    def progress(self, progress):
        return self._request('progress', progress=progress)

    def references(self, context):
        return self._request('references', context=context)

    def offers(self, offers):
        return self._request('offers', offers=offers)

    def respond(self, recipient, revision, confirm, terms, allowed=()):
        return self._request('respond', recipient=recipient, revision=revision,
                             confirm=confirm, terms=terms, allowed=list(allowed))

    def clear(self):
        return self._request('clear')

    def close(self):
        return self._request('drop')
