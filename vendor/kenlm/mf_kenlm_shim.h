#ifndef MF_KENLM_SHIM_H
#define MF_KENLM_SHIM_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stddef.h>
#include <stdint.h>

typedef struct MfKenlmModel MfKenlmModel;

/* Load a binary or ARPA KenLM model. Returns NULL on failure. */
MfKenlmModel *mf_kenlm_load(const char *path);

/* Free the model and release all resources. */
void mf_kenlm_free(MfKenlmModel *model);

/* Return the n-gram order (e.g. 5). */
unsigned char mf_kenlm_order(const MfKenlmModel *model);

/* Score a whitespace-tokenized sentence and return the total log10
 * probability (including </s>). Returns quiet NaN on error. */
double mf_kenlm_score(const MfKenlmModel *model, const char *sentence);

/* Perplexity of a whitespace-tokenized sentence (token count excludes
 * begin-of-sentence but includes end-of-sentence). Returns quiet NaN on
 * error or empty input. */
double mf_kenlm_perplexity(const MfKenlmModel *model, const char *sentence);

/* Returns 1 if every token in the whitespace-delimited sentence is in
 * the model vocabulary, 0 otherwise. */
int mf_kenlm_contains_all(const MfKenlmModel *model, const char *sentence);

/* Return a version string (e.g. "kenlm-4cb443e"). Caller does NOT own
 * the returned pointer. */
const char *mf_kenlm_version(void);

#ifdef __cplusplus
}
#endif

#endif /* MF_KENLM_SHIM_H */
