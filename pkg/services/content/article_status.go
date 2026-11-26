package content

import (
	"fmt"

	"github.com/alswl/mind-forge/pkg/common/model"
	output "github.com/alswl/mind-forge/pkg/common/outcome"
)

func validateArticleStatus(status *string) error {
	if status != nil && model.ValidateArticleStatus(*status) != nil {
		return output.Usage(fmt.Sprintf("invalid status '%s'", *status), "use --status draft, published, completed, or abandoned")
	}
	return nil
}
