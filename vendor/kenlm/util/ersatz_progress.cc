#include "util/ersatz_progress.hh"

#include <ostream>
#include <string>

namespace util {

const char kProgressBanner[] = "";

ErsatzProgress::ErsatzProgress()
    : current_(0), next_((uint64_t)-1), complete_(0), stones_written_(0), out_(nullptr) {}

ErsatzProgress::ErsatzProgress(uint64_t complete, std::ostream *to, const std::string & /*message*/)
    : current_(0), next_((uint64_t)-1), complete_(complete), stones_written_(0), out_(to) {}

ErsatzProgress::~ErsatzProgress() {}

void ErsatzProgress::Milestone() {
  // No-op: progress reporting is not needed in query-only mode.
}

} // namespace util
