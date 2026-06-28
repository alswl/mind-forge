#include "lm/bhiksha.hh"
#include "lm/binary_format.hh"
#include "lm/config.hh"

#include <stdint.h>
#include <cassert>

namespace lm {
namespace ngram {
namespace trie {

// DontBhiksha — no-op implementation (no actual bhiksha compression).

DontBhiksha::DontBhiksha(const void * /*base*/, uint64_t /*max_offset*/, uint64_t /*max_next*/,
                         const Config & /*config*/) {}

// ArrayBhiksha — stub implementations for build-time methods that are never
// called during query-only operation.

void ArrayBhiksha::UpdateConfigFromBinary(const BinaryFormat & /*file*/, uint64_t /*offset*/,
                                          Config & /*config*/) {}

uint64_t ArrayBhiksha::Size(uint64_t /*max_offset*/, uint64_t /*max_next*/, const Config & /*config*/) {
  return 0;
}

uint8_t ArrayBhiksha::InlineBits(uint64_t /*max_offset*/, uint64_t max_next, const Config & /*config*/) {
  return util::RequiredBits(max_next);
}

ArrayBhiksha::ArrayBhiksha(void * /*base*/, uint64_t /*max_offset*/, uint64_t /*max_value*/,
                           const Config & /*config*/)
    : next_inline_(), offset_begin_(nullptr), offset_end_(nullptr), write_to_(nullptr),
      original_base_(nullptr) {}

void ArrayBhiksha::FinishedLoading(const Config & /*config*/) {}

} // namespace trie
} // namespace ngram
} // namespace lm
