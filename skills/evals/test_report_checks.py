import unittest

from check import renderer_renders_delivery, report_contains_delivery


class DeliveryEvidenceTest(unittest.TestCase):
    def test_fallback_value_and_registry_slug_are_insufficient(self):
        self.assertFalse(renderer_renders_delivery('return <p>{data.completed} requests</p>'))
        self.assertFalse(report_contains_delivery('<p>7 requests</p><!-- repo/delivery -->'))
        self.assertFalse(report_contains_delivery(
            '<section data-report-component="repo/delivery">Delivery completion</section><p>7 requests</p>'
        ))
        self.assertFalse(report_contains_delivery(
            '<section data-report-component="repo/delivery">Delivery completion'
            '<span hidden>7 requests</span><p>17 requests</p></section>'
        ))

    def test_lookup_must_invoke_the_returned_component(self):
        lookup = 'const Delivery = components.get("repo/delivery");'
        self.assertFalse(renderer_renders_delivery(lookup + 'return <p>7 requests</p>;'))
        self.assertFalse(renderer_renders_delivery('// ' + lookup + '\nreturn <Delivery value={7} />;'))
        self.assertFalse(renderer_renders_delivery(
            lookup + 'return <div data-report-component="repo/delivery"><Delivery value={7} /></div>;'
        ))
        self.assertTrue(renderer_renders_delivery(lookup + 'return <Delivery value={7} />;'))

    def test_checks_visible_content_inside_registered_component(self):
        self.assertTrue(report_contains_delivery(
            '<main><dl data-report-component="repo/delivery"><dt>Delivery completion</dt>'
            '<dd><strong>7</strong> requests</dd></dl></main>'
        ))
