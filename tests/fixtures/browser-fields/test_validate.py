import copy
import json
from pathlib import Path
import unittest
from validate import validate

ROOT=Path(__file__).parent

class FixtureContract(unittest.TestCase):
    def setUp(self):
        self.html=(ROOT/"index.html").read_text()
        self.matrix=json.loads((ROOT/"matrix.json").read_text())

    def test_complete_public_fixture_and_twenty_prospective_workflows(self):
        self.assertEqual(validate(self.html,self.matrix),{"htmlCases":18,"workflows":20,"nativeExecutionPerformed":False})

    def test_network_target_duplicate_id_and_password_initial_value_reject(self):
        mutations=(self.html.replace('href="#positive"','href="https://example.invalid/"'),
                   self.html.replace('id="editable-other"','id="editable-input"'),
                   self.html.replace('id="password-input" data-case="S01"','id="password-input" value="secret" data-case="S01"'))
        for html in mutations:
            with self.assertRaises(ValueError):
                validate(html,self.matrix)

    def test_aria_only_negative_is_not_disguised_as_html_readonly(self):
        html=self.html.replace('id="aria-input" data-case="N03"', 'id="aria-input" readonly data-case="N03"')
        with self.assertRaises(ValueError):
            validate(html,self.matrix)

    def test_unknown_missing_or_claimed_pass_rows_reject(self):
        for mutate in (lambda m:m["cases"].pop(),
                       lambda m:m["cases"][0].update(status="passed"),
                       lambda m:m.update(nativeExecutionPerformed=True)):
            matrix=copy.deepcopy(self.matrix)
            mutate(matrix)
            with self.assertRaises(ValueError):
                validate(self.html,matrix)

    def test_input_reading_or_telemetry_in_actual_script_reject(self):
        for injection in ('console.log(document.getElementById("password-input").value);',
                          'fetch("https://example.invalid/");',
                          'localStorage.setItem("x","y");'):
            html=self.html.replace('<script id="fixture-code">','<script id="fixture-code">'+injection)
            with self.assertRaises(ValueError):
                validate(html,self.matrix)

if __name__=="__main__":
    unittest.main()
