#ifndef GAMECTX_NATIVE_H
#define GAMECTX_NATIVE_H
#include <stddef.h>
#ifdef __cplusplus
extern "C" {
#endif
typedef struct GamectxProject GamectxProject;
typedef struct GamectxSession GamectxSession;
/* Optional persistent character sessions; legacy immutable project handles are unchanged. */
GamectxSession *gamectx_session_new(const unsigned char *bytes, size_t len);
char *gamectx_session_request(const GamectxSession *, const unsigned char *bytes, size_t len);
void gamectx_session_free(GamectxSession *);
/* UTF-8 buffers need not end in NUL. Null return: call gamectx_last_error(). */
GamectxProject *gamectx_project_new(const unsigned char *bytes, size_t len);
/* Handle stays alive throughout all requests. Immutable requests may run concurrently. */
char *gamectx_project_request(const GamectxProject *, const unsigned char *bytes, size_t len);
char *gamectx_last_error(void);
/* Free each returned string exactly once with this function, never the host allocator. */
void gamectx_string_free(char *);
/* Wait for all requests to finish before freeing the handle. */
void gamectx_project_free(GamectxProject *);
#ifdef __cplusplus
}
#endif
#endif
