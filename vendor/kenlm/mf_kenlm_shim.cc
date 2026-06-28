#include "mf_kenlm_shim.h"
#include "lm/model.hh"

#include <cmath>
#include <cstdlib>
#include <iostream>
#include <string>
#include <sstream>
#include <vector>

struct MfKenlmModel {
    lm::base::Model *model;
};

static const char kVersion[] = "kenlm-4cb443e";

extern "C" {

MfKenlmModel *mf_kenlm_load(const char *path) {
    MfKenlmModel *m = new MfKenlmModel;
    m->model = nullptr;
    try {
        lm::ngram::Config config;
        // Use lazy loading + mmap for performance; never populate the
        // probing hash table eagerly (FR-L1, R3).
        config.load_method = util::LAZY;
        m->model = lm::ngram::LoadVirtual(path, config);
        if (!m->model) {
            delete m;
            return nullptr;
        }
        return m;
    } catch (const std::exception &e) {
        std::cerr << "mf_kenlm_load error: " << e.what() << std::endl;
        delete m;
        return nullptr;
    } catch (...) {
        std::cerr << "mf_kenlm_load: unknown error" << std::endl;
        delete m;
        return nullptr;
    }
}

void mf_kenlm_free(MfKenlmModel *m) {
    if (!m) return;
    delete m->model;
    delete m;
}

unsigned char mf_kenlm_order(const MfKenlmModel *m) {
    if (!m || !m->model) return 0;
    return m->model->Order();
}

static std::vector<std::string> tokenize(const char *sentence) {
    std::vector<std::string> tokens;
    std::istringstream iss(sentence ? sentence : "");
    std::string tok;
    while (iss >> tok) {
        tokens.push_back(tok);
    }
    return tokens;
}

double mf_kenlm_score(const MfKenlmModel *m, const char *sentence) {
    if (!m || !m->model || !sentence) return std::nan("");
    try {
        std::vector<std::string> words = tokenize(sentence);
        if (words.empty()) return std::nan("");

        lm::base::Model *model = m->model;
        const lm::base::Vocabulary &vocab = model->BaseVocabulary();

        double total = 0.0;
        // Allocate state buffers
        size_t state_size = model->StateSize();
        std::vector<uint8_t> state_buf(state_size);
        void *in_state = malloc(state_size);
        void *out_state = malloc(state_size);
        if (!in_state || !out_state) {
            free(in_state);
            free(out_state);
            return std::nan("");
        }

        model->BeginSentenceWrite(in_state);

        for (const auto &w : words) {
            lm::WordIndex idx = vocab.Index(w);
            total += static_cast<double>(model->BaseScore(in_state, idx, out_state));
            std::swap(in_state, out_state);
        }
        // End-of-sentence
        total += static_cast<double>(model->BaseScore(in_state, vocab.EndSentence(), out_state));

        free(in_state);
        free(out_state);
        return total;
    } catch (...) {
        return std::nan("");
    }
}

double mf_kenlm_perplexity(const MfKenlmModel *m, const char *sentence) {
    if (!m || !m->model || !sentence) return std::nan("");
    try {
        std::vector<std::string> words = tokenize(sentence);
        if (words.empty()) return std::nan("");

        lm::base::Model *model = m->model;
        const lm::base::Vocabulary &vocab = model->BaseVocabulary();

        double total = 0.0;
        size_t state_size = model->StateSize();
        void *in_state = malloc(state_size);
        void *out_state = malloc(state_size);
        if (!in_state || !out_state) {
            free(in_state);
            free(out_state);
            return std::nan("");
        }

        model->BeginSentenceWrite(in_state);
        unsigned int token_count = 0;

        for (const auto &w : words) {
            lm::WordIndex idx = vocab.Index(w);
            total += static_cast<double>(model->BaseScore(in_state, idx, out_state));
            std::swap(in_state, out_state);
            ++token_count;
        }
        // End-of-sentence token
        total += static_cast<double>(model->BaseScore(in_state, vocab.EndSentence(), out_state));
        ++token_count;

        free(in_state);
        free(out_state);

        if (token_count == 0) return std::nan("");
        // Perplexity = 10^(-total / token_count)
        return std::pow(10.0, -total / static_cast<double>(token_count));
    } catch (...) {
        return std::nan("");
    }
}

int mf_kenlm_contains_all(const MfKenlmModel *m, const char *sentence) {
    if (!m || !m->model || !sentence) return 0;
    try {
        std::vector<std::string> words = tokenize(sentence);
        if (words.empty()) return 1;
        const lm::base::Vocabulary &vocab = m->model->BaseVocabulary();
        for (const auto &w : words) {
            if (vocab.Index(w) == vocab.NotFound()) {
                return 0;
            }
        }
        return 1;
    } catch (...) {
        return 0;
    }
}

const char *mf_kenlm_version(void) {
    return kVersion;
}

} // extern "C"
