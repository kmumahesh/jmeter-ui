# Test scenarios

Every automated test and real-window check that exists today, one scenario per line. The name of the test is shown in `code font`.

## What should work

- **Plans open and save safely.** Any `.jmx` file opens, and a saved file keeps every untouched byte exactly as it was. Missing or invalid files give a clear message, not a crash.
- **Every change can be undone.** Add, delete, duplicate, move, rename, enable or disable, and every setting change undo and redo. Undoing everything gives back the original file.
- **Elements follow JMeter's rules.** Only elements JMeter allows can be added or dropped where you choose. A plan with one of every element loads in real JMeter.
- **Forms edit everything.** Every element has a form. Text, numbers (including `${variables}`), checkboxes, drop-downs, flag groups and tables all save to the plan, and typing shows as unsaved at once.
- **Work is never lost silently.** Unsaved marks, a recovery copy while you work, and a question before closing, quitting or reloading.
- **Damaged input is survived.** Real plans and results files with random damage (flipped bytes, cut and doubled pieces, stray markup, truncation) never crash or hang the readers; every plan the reader accepts is saved back exactly as it came; a results file written in pieces reads the same as the whole file. The same checks run for much longer as libFuzzer targets on GitHub (`fuzz/`, `.github/workflows/fuzz.yml`).
- **Hostile files cannot do harm.** A plan with an entity trick, absurd nesting, a huge size or damaged text is refused with a message or read safely, never read from another file, never fetched from the web, never a crash. Any text typed into a name, comment, value or table cell stays text, and saving never writes to a file it should not.
- **Runs work and stay under control.** Run (F5), Validate (one user, once) and Send once (one request) run the saved plan, asking to save first and asking once per plan file before running scripts. Live numbers, percentiles, charts, the failed requests and a table per request update while it runs. A run stops gracefully, at once or by force, and JMeter never keeps running after the app closes or crashes.
- **Run files are kept as chosen.** Each run has its own folder inside a `runs` folder, next to the `.jmx` file or inside a folder chosen in Settings. Only the last N runs of a plan are kept, with all their files; earlier runs can be looked at again, and the shared folder can be cleared without touching the runs next to a plan.
- **Running is safe.** Nothing typed into a path, a property or a setting can add to the JMeter command line, the stop port answers this machine only, a flood of output or a hostile results file cannot use up memory, and only folders this app made are ever removed.
- **The window behaves.** One tab per plan, the tree drives the editor, and the palette, shortcuts, drag and drop and right-click menu work. The keyboard focus stays where you are typing.
- **The extras are correct.** Scope & Order matches how JMeter runs things, and `${variables}` that nothing defines are flagged.
- **Settings and looks persist.** Themes, panel sizes, window size, recent files and the Java and JMeter paths are remembered, and a damaged settings file is set aside.
- **Big plans stay usable.** A 20,000-element plan opens and edits quickly.

The right-click menu can only be tested in the real window (section 3), because the headless window cannot open pop-up menus.

## How to run each kind

- Unit tests and whole-window tests: `cargo test --workspace --exclude gpui-spike --exclude zed-ui-spike`
- Real-window checks: `cargo build -p jmeter-ui`, then `powershell -ExecutionPolicy Bypass -File tests\ui\smoke.ps1`
- Everything at once, with the time each part took written to `docs/TEST-TIMINGS.md`: `powershell -ExecutionPolicy Bypass -File tools\run-tests.ps1` (add `-Smoke` to include the real-window script, which takes over the mouse and keyboard).
- This list is rebuilt from the test sources with `python tools/gen-test-doc.py`; the summary above is written by hand.
- On GitHub (`.github/workflows/ci.yml`), every push and pull request installs Java 17 and JMeter 5.6.3 (download checked against Apache's SHA-512) so the tests that run the real JMeter are not skipped. The real-window checks run in their own workflow, `.github/workflows/ui-smoke.yml` (every pull request and push to `main`): the runner screen is raised to 1920x1080 first, and all 61 checks pass there in about 6.5 minutes.
- Linux and macOS: the same automated tests run there in CI (`Build and test (ubuntu-latest)`, `Build and test (macos-latest)`), including the ones that start the real JMeter. Text shortcuts in the tests use `secondary(...)` in `crates/app/tests/common/mod.rs`, which turns Ctrl into Cmd on macOS. The check that JMeter is ended when the app crashes asserts on Windows only.
- Fuzzing (Linux, nightly compiler, about a minute per target): `cargo install cargo-fuzz`, then `cargo +nightly fuzz run jmx_parse` (also `jmx_text`, `plan_load`, `results_jtl`) from the repo root; it runs on GitHub for changes to the readers. Test coverage: `.github/workflows/coverage.yml` (every push to `main`) keeps a summary, an lcov file and an HTML report.
- Supply-chain checks (known vulnerabilities, licences, code sources): `cargo audit` and `cargo deny check`; the same runs on GitHub in `.github/workflows/ci.yml`.
- Some unit tests read sample plans from the JMeter install (set `JMX_CORPUS` / `JMETER_HOME`); they skip themselves when it is missing.

## 1. Unit tests

These test the logic without a window. Tests that start the real JMeter skip themselves when Java or JMeter is not installed.

### jmx-model (reading, editing and writing `.jmx` text)

*crates/jmx-model/src/edit.rs*

1. Indexing gives every plan element a distinct uid and changes nothing. — `indexing_gives_every_plan_element_a_distinct_uid_and_changes_nothing`
2. Positions and children. — `positions_and_children`
3. Remove and restore is byte identical everywhere. — `remove_and_restore_is_byte_identical_everywhere`
4. Removing the last child collapses the hash tree and restores. — `removing_the_last_child_collapses_the_hash_tree_and_restores`
5. Inserting matches the surrounding indentation. — `inserting_matches_the_surrounding_indentation`
6. Inserting at the end and into an empty level. — `inserting_at_the_end_and_into_an_empty_level`
7. Copies get new uids and keep their children. — `copies_get_new_uids_and_keep_their_children`
8. Set prop changes adds and restores. — `set_prop_changes_adds_and_restores`
9. Restoring keeps odd original text. — `restoring_keeps_odd_original_text`
10. Settings inside an element property are reached with a path. — `settings_inside_an_element_property_are_reached_with_a_path`
11. A number setting given a variable becomes a string setting. — `a_number_setting_given_a_variable_becomes_a_string_setting`
12. Set prop on an empty element. — `set_prop_on_an_empty_element`
13. Attributes are escaped. — `attributes_are_escaped`
14. Crlf files get crlf in new elements. — `crlf_files_get_crlf_in_new_elements`
15. Missing things are errors not panics. — `missing_things_are_errors_not_panics`
16. Every setting text of an element is listed with its path. — `every_setting_text_of_an_element_is_listed_with_its_path`
17. Rows can be read by position. — `rows_can_be_read_by_position`
18. Editing a cell goes through set prop and restores. — `editing_a_cell_goes_through_set_prop_and_restores`
19. Inserting a row matches the indentation and undoes exactly. — `inserting_a_row_matches_the_indentation_and_undoes_exactly`
20. Removing every row collapses the table and restoring expands it. — `removing_every_row_collapses_the_table_and_restoring_expands_it`
21. A row can be added to an empty table. — `a_row_can_be_added_to_an_empty_table`
22. Bad table paths are errors. — `bad_table_paths_are_errors`
*crates/jmx-model/src/plan.rs*

23. Reads simple collection and nested props. — `reads_simple_collection_and_nested_props`
*crates/jmx-model/tests/mutations.rs*

24. Damaged plans never crash the reader and what it accepts is saved back exactly. — `damaged_plans_never_crash_the_reader_and_what_it_accepts_is_saved_back_exactly`
25. Damaged fragments and text never crash and escaping round trips. — `damaged_fragments_and_text_never_crash_and_escaping_round_trips`
*crates/jmx-model/tests/runcopy.rs*

26. Injects one collector under the test plan. — `injects_one_collector_under_the_test_plan`
27. The path is escaped. — `the_path_is_escaped`
28. Everything before the insertion point is unchanged. — `everything_before_the_insertion_point_is_unchanged`
29. Rejects documents without a test plan. — `rejects_documents_without_a_test_plan`
30. Sets loops everywhere and counts. — `sets_loops_everywhere_and_counts`
*crates/jmx-model/tests/security.rs*

31. A doctype with an external entity is kept as text and never read. — `a_doctype_with_an_external_entity_is_kept_as_text_and_never_read`
32. An entity that points at a web address is never fetched. — `an_entity_that_points_at_a_web_address_is_never_fetched`
33. Entity expansion bombs do not grow. — `entity_expansion_bombs_do_not_grow`
34. Character references never make invalid text. — `character_references_never_make_invalid_text`
35. Deep nesting is refused with an error. — `deep_nesting_is_refused_with_an_error`
36. Nesting deeper than any real plan still opens. — `nesting_deeper_than_any_real_plan_still_opens`
37. A very wide plan opens quickly. — `a_very_wide_plan_opens_quickly`
38. A very long text value round trips. — `a_very_long_text_value_round_trips`
39. Truncating a plan at any point gives an error or a plan never a panic. — `truncating_a_plan_at_any_point_gives_an_error_or_a_plan_never_a_panic`
40. Damaged documents are errors. — `damaged_documents_are_errors`
41. Mutated plans never panic and always write back what was read. — `mutated_plans_never_panic_and_always_write_back_what_was_read`
42. Hostile text in an element stays text. — `hostile_text_in_an_element_stays_text`
43. Hostile text in an attribute stays in that attribute. — `hostile_text_in_an_attribute_stays_in_that_attribute`
44. Escaping never leaves a character that can end the value. — `escaping_never_leaves_a_character_that_can_end_the_value`
*crates/jmx-model/tests/xml.rs*

45. Unchanged documents are written back exactly. — `unchanged_documents_are_written_back_exactly`
46. Attributes are decoded on read. — `attributes_are_decoded_on_read`
47. Editing an attribute rewrites only that start tag. — `editing_an_attribute_rewrites_only_that_start_tag`
48. Adding a new attribute. — `adding_a_new_attribute`
49. Set text escapes and expands self closing elements. — `set_text_escapes_and_expands_self_closing_elements`
50. Text reading joins runs and decodes. — `text_reading_joins_runs_and_decodes`
51. Pushing children into a self closing element. — `pushing_children_into_a_self_closing_element`
52. Walking visits every element depth first. — `walking_visits_every_element_depth_first`
53. Elements skips text and comments. — `elements_skips_text_and_comments`
54. Malformed input is an error not a panic. — `malformed_input_is_an_error_not_a_panic`
55. Escape and unescape are inverse. — `escape_and_unescape_are_inverse`
56. Unknown entities are left alone. — `unknown_entities_are_left_alone`
57. Deeply nested documents work. — `deeply_nested_documents_work`
58. Fragments parse to one element. — `fragments_parse_to_one_element`

### plan-core (elements, rules, editing, forms, scope, variables, saving, scripts, plans made for Validate and Send once)

*crates/plan-core/src/catalog.rs*

59. Table is sorted and unique. — `table_is_sorted_and_unique`
60. Core elements. — `core_elements`
61. Shared tags use the gui class. — `shared_tags_use_the_gui_class`
62. Plugin classes are guessed from their names. — `plugin_classes_are_guessed_from_their_names`
63. Unknown core tags get a readable name. — `unknown_core_tags_get_a_readable_name`
64. Humanize handles acronyms. — `humanize_handles_acronyms`
65. Every category has an icon and labels. — `every_category_has_an_icon_and_labels`
66. List is complete and unique. — `list_is_complete_and_unique`
67. Variants are expanded. — `variants_are_expanded`
68. Every quick add entry exists. — `every_quick_add_entry_exists`
69. Sorted by category then name. — `sorted_by_category_then_name`
70. Elements missing from the table are found through jmeters menus. — `elements_missing_from_the_table_are_found_through_jmeters_menus`
71. Every menu element is classified the same way by both sources. — `every_menu_element_is_classified_the_same_way_by_both_sources`
*crates/plan-core/src/derive.rs*

72. Validate limits every group and drops timers and backend listeners. — `validate_limits_every_group_and_drops_timers_and_backend_listeners`
73. Validate keeps every group and every sampler. — `validate_keeps_every_group_and_every_sampler`
74. Validate needs a thread group. — `validate_needs_a_thread_group`
75. Send once keeps the request and what applies to it in jmeters order. — `send_once_keeps_the_request_and_what_applies_to_it_in_jmeters_order`
76. Send once can send a request with nothing else applying. — `send_once_can_send_a_request_with_nothing_else_applying`
77. Send once refuses what is not a plain request. — `send_once_refuses_what_is_not_a_plain_request`
78. Send once skips requests under a test fragment. — `send_once_skips_requests_under_a_test_fragment`
79. Relative file names become absolute and others are left alone. — `relative_file_names_become_absolute_and_others_are_left_alone`
80. Hostile text in names and files survives the derived plan. — `hostile_text_in_names_and_files_survives_the_derived_plan`
*crates/plan-core/src/edit.rs*

81. A fresh plan is clean and writes back unchanged. — `a_fresh_plan_is_clean_and_writes_back_unchanged`
82. Adding puts the element last and gives it jmeters defaults. — `adding_puts_the_element_last_and_gives_it_jmeters_defaults`
83. The rules decide what can be added where. — `the_rules_decide_what_can_be_added_where`
84. Undo and redo restore the text exactly. — `undo_and_redo_restore_the_text_exactly`
85. Removing and undoing keeps the same ids. — `removing_and_undoing_keeps_the_same_ids`
86. The test plan cannot be removed. — `the_test_plan_cannot_be_removed`
87. Duplicate copies children and sits after the original. — `duplicate_copies_children_and_sits_after_the_original`
88. Moving reorders and reparents and undoes. — `moving_reorders_and_reparents_and_undoes`
89. Moving between levels re indents and undoes exactly. — `moving_between_levels_re_indents_and_undoes_exactly`
90. Settings changes are undoable and a no op is not recorded. — `settings_changes_are_undoable_and_a_no_op_is_not_recorded`
91. Disabling shows in the plan. — `disabling_shows_in_the_plan`
92. Dirty follows saves and history. — `dirty_follows_saves_and_history`
93. A new edit clears redo. — `a_new_edit_clears_redo`
94. Missing elements are errors. — `missing_elements_are_errors`
*crates/plan-core/src/files.rs*

95. The recovery copy sits next to the plan. — `the_recovery_copy_sits_next_to_the_plan`
96. Save replaces the file and removes the recovery copy. — `save_replaces_the_file_and_removes_the_recovery_copy`
97. A failed save leaves the old file. — `a_failed_save_leaves_the_old_file`
98. A leftover copy is found when it differs. — `a_leftover_copy_is_found_when_it_differs`
99. A copy identical to the file is not a recovery. — `a_copy_identical_to_the_file_is_not_a_recovery`
100. A copy without its plan is still offered. — `a_copy_without_its_plan_is_still_offered`
101. An older copy is marked as not newer. — `an_older_copy_is_marked_as_not_newer`
*crates/plan-core/src/forms.rs*

102. Every curated form names settings that jmeter knows. — `every_curated_form_names_settings_that_jmeter_knows`
103. Forms exist for the everyday elements. — `forms_exist_for_the_everyday_elements`
104. Curated forms cover most of jmeters menus. — `curated_forms_cover_most_of_jmeters_menus`
105. Field properties are unique within a form except for flags in one setting. — `field_properties_are_unique_within_a_form_except_for_flags_in_one_setting`
106. Choices include the default and have unique values. — `choices_include_the_default_and_have_unique_values`
107. Every table path exists in a new element or is known. — `every_table_path_exists_in_a_new_element_or_is_known`
108. The generic form covers every simple setting. — `the_generic_form_covers_every_simple_setting`
109. Labels are made readable. — `labels_are_made_readable`
110. Java hash matches java. — `java_hash_matches_java`
111. Rows of every shape parse and escape. — `rows_of_every_shape_parse_and_escape`
112. Parameters can be added edited removed and undone. — `parameters_can_be_added_edited_removed_and_undone`
113. Response assertion patterns are a list of strings. — `response_assertion_patterns_are_a_list_of_strings`
114. Bad tables are errors. — `bad_tables_are_errors`
*crates/plan-core/src/fuzzy.rs*

115. Empty query keeps everything in order. — `empty_query_keeps_everything_in_order`
116. Matches subsequences and reports positions. — `matches_subsequences_and_reports_positions`
117. Better matches rank first. — `better_matches_rank_first`
118. Lower case ignores case and upper case does not. — `lower_case_ignores_case_and_upper_case_does_not`
119. Exact prefix beats scattered match. — `exact_prefix_beats_scattered_match`
120. Ties keep input order. — `ties_keep_input_order`
*crates/plan-core/src/model.rs*

121. Builds the tree in document order. — `builds_the_tree_in_document_order`
122. Empty names fall back to the type label. — `empty_names_fall_back_to_the_type_label`
123. Parents children and paths. — `parents_children_and_paths`
124. Categories flags and summary. — `categories_flags_and_summary`
125. Move rules and cycles. — `move_rules_and_cycles`
126. Rejects malformed plans. — `rejects_malformed_plans`
127. Missing file error names the path. — `missing_file_error_names_the_path`
128. Props and comments are carried over. — `props_and_comments_are_carried_over`
*crates/plan-core/src/problems.rs*

129. A healthy plan has no problems. — `a_healthy_plan_has_no_problems`
130. No thread group. — `no_thread_group`
131. Thread group without requests. — `thread_group_without_requests`
132. Requests nested in controllers count. — `requests_nested_in_controllers_count`
133. Disabled thread group is a note not a warning. — `disabled_thread_group_is_a_note_not_a_warning`
134. Plugin and unknown elements are grouped by class. — `plugin_and_unknown_elements_are_grouped_by_class`
135. Disabled elements are counted. — `disabled_elements_are_counted`
136. Empty plan gives nothing. — `empty_plan_gives_nothing`
*crates/plan-core/src/rules.rs*

137. Test plan and workbench are never children. — `test_plan_and_workbench_are_never_children`
138. Thread groups and fragments only under the plan. — `thread_groups_and_fragments_only_under_the_plan`
139. Non test elements go under plan fragment or workbench. — `non_test_elements_go_under_plan_fragment_or_workbench`
140. Plan accepts config timers listeners but not samplers or controllers. — `plan_accepts_config_timers_listeners_but_not_samplers_or_controllers`
141. Controllers and thread groups accept samplers and controllers. — `controllers_and_thread_groups_accept_samplers_and_controllers`
142. Workbench accepts everything except thread groups and structure. — `workbench_accepts_everything_except_thread_groups_and_structure`
143. Samplers accept processors but not samplers or controllers. — `samplers_accept_processors_but_not_samplers_or_controllers`
144. Recorder accepts processors but not samplers. — `recorder_accepts_processors_but_not_samplers`
145. Leaf categories accept nothing. — `leaf_categories_accept_nothing`
146. Unknown parent is permissive but still blocks structural children. — `unknown_parent_is_permissive_but_still_blocks_structural_children`
147. Allowed children lists. — `allowed_children_lists`
*crates/plan-core/src/scope.rs*

148. A sampler gets its steps in jmeters order. — `a_sampler_gets_its_steps_in_jmeters_order`
149. Configuration is nearest first and the others follow their note. — `configuration_is_nearest_first_and_the_others_follow_their_note`
150. An element under a sibling sampler does not apply. — `an_element_under_a_sibling_sampler_does_not_apply`
151. Something under a sampler applies to that sampler only. — `something_under_a_sampler_applies_to_that_sampler_only`
152. Something under a controller applies to every sampler inside its parent. — `something_under_a_controller_applies_to_every_sampler_inside_its_parent`
153. A sampler applies to itself and a controller to nothing in particular. — `a_sampler_applies_to_itself_and_a_controller_to_nothing_in_particular`
154. A controller lists what it shares with everything inside. — `a_controller_lists_what_it_shares_with_everything_inside`
155. Switched off elements are listed but inactive. — `switched_off_elements_are_listed_but_inactive`
156. Nothing runs inside a test fragment. — `nothing_runs_inside_a_test_fragment`
157. Non samplers have no run order. — `non_samplers_have_no_run_order`
*crates/plan-core/src/scripts.rs*

158. A plain plan has no scripts. — `a_plain_plan_has_no_scripts`
159. Script elements are found by kind. — `script_elements_are_found_by_kind`
160. Script functions are found wherever they are written. — `script_functions_are_found_wherever_they_are_written`
161. The question names how many places and what kinds. — `the_question_names_how_many_places_and_what_kinds`
162. Every corpus plan is scanned without trouble. — `every_corpus_plan_is_scanned_without_trouble`
*crates/plan-core/src/search.rs*

163. Finds by name. — `finds_by_name`
164. Finds by type when the name does not match. — `finds_by_type_when_the_name_does_not_match`
165. Name matches rank above type matches. — `name_matches_rank_above_type_matches`
166. Empty query and limit. — `empty_query_and_limit`
*crates/plan-core/src/templates.rs*

167. Every menu entry loads. — `every_menu_entry_loads`
168. Entries are unique by tag and gui. — `entries_are_unique_by_tag_and_gui`
169. Menu groups match the catalog. — `menu_groups_match_the_catalog`
170. Http request has what the gui gives it. — `http_request_has_what_the_gui_gives_it`
171. Listeners with the same tag are told apart by gui. — `listeners_with_the_same_tag_are_told_apart_by_gui`
*crates/plan-core/src/tree.rs*

172. Collapsed tree shows only roots. — `collapsed_tree_shows_only_roots`
173. Expanded to depth. — `expanded_to_depth`
174. Toggle and row flags. — `toggle_and_row_flags`
175. Select expands ancestors. — `select_expands_ancestors`
176. Collapsing an ancestor moves the selection to it. — `collapsing_an_ancestor_moves_the_selection_to_it`
177. Expand all and collapse all. — `expand_all_and_collapse_all`
178. Arrow navigation stops at the ends. — `arrow_navigation_stops_at_the_ends`
179. Up with no selection picks the last row and pages clamp. — `up_with_no_selection_picks_the_last_row_and_pages_clamp`
180. Left and right keys. — `left_and_right_keys`
181. Selecting a missing node is ignored. — `selecting_a_missing_node_is_ignored`
*crates/plan-core/src/variables.rs*

182. Names come from settings tables and the test plan. — `names_come_from_settings_tables_and_the_test_plan`
183. Scripts can define variables. — `scripts_can_define_variables`
184. Only undefined names are reported once per element. — `only_undefined_names_are_reported_once_per_element`
185. What an extractor adds to a name counts as defined. — `what_an_extractor_adds_to_a_name_counts_as_defined`
186. Functions and odd text are left alone. — `functions_and_odd_text_are_left_alone`
187. Built in names are known. — `built_in_names_are_known`
188. A plan in which everything is defined has no hints. — `a_plan_in_which_everything_is_defined_has_no_hints`
*crates/plan-core/tests/corpus.rs*

189. Simple jmeter sample. — `simple_jmeter_sample`
190. Web test template has the usual structure. — `web_test_template_has_the_usual_structure`
191. Plugin plan keeps plugin elements. — `plugin_plan_keeps_plugin_elements`
192. Every real parent child pair obeys the rules. — `every_real_parent_child_pair_obeys_the_rules`
193. Every core tag in the catalog has a label and icon. — `every_core_tag_in_the_catalog_has_a_label_and_icon`
194. A very large plan loads and expands quickly. — `a_very_large_plan_loads_and_expands_quickly`
*crates/plan-core/tests/derive_run.rs*

195. Validate runs every sampler once with one user and no waiting. — `validate_runs_every_sampler_once_with_one_user_and_no_waiting`
196. Send once sends exactly one request. — `send_once_sends_exactly_one_request`
197. Send once of each request in a plan works. — `send_once_of_each_request_in_a_plan_works`
*crates/plan-core/tests/edit_corpus.rs*

198. Random edits on real plans stay valid and undo exactly. — `random_edits_on_real_plans_stay_valid_and_undo_exactly`
199. Editing a large plan is fast enough. — `editing_a_large_plan_is_fast_enough`
*crates/plan-core/tests/jmeter_load.rs*

200. A plan with every element loads in jmeter. — `a_plan_with_every_element_loads_in_jmeter`
*crates/plan-core/tests/mutations.rs*

201. Damaged plans load or are refused and a loaded plan can always be walked. — `damaged_plans_load_or_are_refused_and_a_loaded_plan_can_always_be_walked`
*crates/plan-core/tests/security.rs*

202. Hostile text through every edit keeps the plan structure. — `hostile_text_through_every_edit_keeps_the_plan_structure`
203. A plan nested as deep as allowed still builds and edits. — `a_plan_nested_as_deep_as_allowed_still_builds_and_edits`
204. A plan nested past the limit is refused with a message. — `a_plan_nested_past_the_limit_is_refused_with_a_message`
205. A file over the size limit is refused without reading it. — `a_file_over_the_size_limit_is_refused_without_reading_it`
206. A file that is not text is an error not a crash. — `a_file_that_is_not_text_is_an_error_not_a_crash`
207. Saving never writes through a leftover temporary file to another file. — `saving_never_writes_through_a_leftover_temporary_file_to_another_file`
208. Values that span several lines survive duplicate move and add. — `values_that_span_several_lines_survive_duplicate_move_and_add`
209. Saving a plan that is a link writes through the link. — `saving_a_plan_that_is_a_link_writes_through_the_link`
210. The recovery copy is written next to this plan only. — `the_recovery_copy_is_written_next_to_this_plan_only`
211. Saving to a read only file fails cleanly and leaves no temporary file. — `saving_to_a_read_only_file_fails_cleanly_and_leaves_no_temporary_file`
212. Saving into a missing folder is an error. — `saving_into_a_missing_folder_is_an_error`
213. A very long path gives an error or works but never panics. — `a_very_long_path_gives_an_error_or_works_but_never_panics`
214. A reserved windows device name is never reported as saved. — `a_reserved_windows_device_name_is_never_reported_as_saved`
215. A recovery copy that is not text is ignored. — `a_recovery_copy_that_is_not_text_is_ignored`

### shell-core (settings, layout, text editing, palette, actions)

*crates/shell-core/src/actions.rs*

216. All lists every action once. — `all_lists_every_action_once`
217. Shortcuts are unique. — `shortcuts_are_unique`
218. Nothing open. — `nothing_open`
219. With a plan and tabs. — `with_a_plan_and_tabs`
220. Next tab needs two tabs. — `next_tab_needs_two_tabs`
221. Run and stop follow the run state. — `run_and_stop_follow_the_run_state`
222. Validate and send once need a plan a runtime and no run going. — `validate_and_send_once_need_a_plan_a_runtime_and_no_run_going`
223. Stop now follows the run state. — `stop_now_follows_the_run_state`
224. Only a chosen runs folder can be cleared and not during a run. — `only_a_chosen_runs_folder_can_be_cleared_and_not_during_a_run`
225. Saving needs unsaved changes and save as only a plan. — `saving_needs_unsaved_changes_and_save_as_only_a_plan`
226. Undo and redo follow the history. — `undo_and_redo_follow_the_history`
227. Element commands need a selection the test plan is special. — `element_commands_need_a_selection_the_test_plan_is_special`
228. Every group has actions. — `every_group_has_actions`
229. Adding follows the edit capability. — `adding_follows_the_edit_capability`
*crates/shell-core/src/layout.rs*

230. Starts empty with panels from settings. — `starts_empty_with_panels_from_settings`
231. The inspector is hidden by default. — `the_inspector_is_hidden_by_default`
232. Opening a tab places it after the active one. — `opening_a_tab_places_it_after_the_active_one`
233. A plan never gets a second tab. — `a_plan_never_gets_a_second_tab`
234. Closing activates the right neighbour then the left. — `closing_activates_the_right_neighbour_then_the_left`
235. Closing a tab before the active one keeps it active. — `closing_a_tab_before_the_active_one_keeps_it_active`
236. Next and previous wrap. — `next_and_previous_wrap`
237. Close others and close all. — `close_others_and_close_all`
238. Rename and dirty apply to the matching tab only. — `rename_and_dirty_apply_to_the_matching_tab_only`
239. Panel sizes clamp and ignore nan. — `panel_sizes_clamp_and_ignore_nan`
240. Toggles and saving back to settings. — `toggles_and_saving_back_to_settings`
*crates/shell-core/src/number.rs*

241. Parses numbers. — `parses_numbers`
242. Rejects bad input with a reason. — `rejects_bad_input_with_a_reason`
243. Variables and functions are valid. — `variables_and_functions_are_valid`
244. Empty is allowed only when optional. — `empty_is_allowed_only_when_optional`
245. Partial input is accepted while typing. — `partial_input_is_accepted_while_typing`
246. Partial expressions are accepted. — `partial_expressions_are_accepted`
247. Stepping. — `stepping`
248. Expressions and junk are not stepped. — `expressions_and_junk_are_not_stepped`
249. Formatting. — `formatting`
250. Clamp handles open ranges. — `clamp_handles_open_ranges`
*crates/shell-core/src/palette.rs*

251. Prefixes pick the mode. — `prefixes_pick_the_mode`
252. Commands mode lists only actions. — `commands_mode_lists_only_actions`
253. Unavailable commands sink to the bottom with a reason. — `unavailable_commands_sink_to_the_bottom_with_a_reason`
254. Elements mode finds by name and carries the type. — `elements_mode_finds_by_name_and_carries_the_type`
255. Elements mode without a query lists the tree in order. — `elements_mode_without_a_query_lists_the_tree_in_order`
256. Plain queries mix commands recents and elements. — `plain_queries_mix_commands_recents_and_elements`
257. An empty query shows commands and a few recents but no elements. — `an_empty_query_shows_commands_and_a_few_recents_but_no_elements`
258. Recent items use the file name as the title. — `recent_items_use_the_file_name_as_the_title`
259. Add mode offers only what fits under the selection. — `add_mode_offers_only_what_fits_under_the_selection`
260. Add mode with a query also lists what does not fit as unavailable. — `add_mode_with_a_query_also_lists_what_does_not_fit_as_unavailable`
261. Add mode quick list comes first without a query. — `add_mode_quick_list_comes_first_without_a_query`
262. Add mode without a selection uses the test plan. — `add_mode_without_a_selection_uses_the_test_plan`
263. Add mode under a leaf offers nothing. — `add_mode_under_a_leaf_offers_nothing`
264. Add items are unavailable until editing exists. — `add_items_are_unavailable_until_editing_exists`
265. Limit is applied after ranking. — `limit_is_applied_after_ranking`
266. Shortcuts are included for commands. — `shortcuts_are_included_for_commands`
*crates/shell-core/src/settings.rs*

267. Missing file gives defaults without a notice. — `missing_file_gives_defaults_without_a_notice`
268. Round trip. — `round_trip`
269. Older files with missing fields still load. — `older_files_with_missing_fields_still_load`
270. Unknown fields are ignored. — `unknown_fields_are_ignored`
271. Damaged file is set aside. — `damaged_file_is_set_aside`
272. Wrong types count as damage. — `wrong_types_count_as_damage`
273. Sizes are clamped on load and save. — `sizes_are_clamped_on_load_and_save`
274. Recent files are unique ordered and capped. — `recent_files_are_unique_ordered_and_capped`
275. A settings file saved with a byte order mark is read. — `a_settings_file_saved_with_a_byte_order_mark_is_read`
276. Run history defaults to the plan folder and ten runs. — `run_history_defaults_to_the_plan_folder_and_ten_runs`
277. Older files without run history settings still load. — `older_files_without_run_history_settings_still_load`
278. Run history settings round trip and are clamped. — `run_history_settings_round_trip_and_are_clamped`
279. Script approval is remembered per plan file and ignores case and slashes. — `script_approval_is_remembered_per_plan_file_and_ignores_case_and_slashes`
280. Default path ends in the app folder. — `default_path_ends_in_the_app_folder`
*crates/shell-core/src/text_buffer.rs*

281. Typing appends at the caret. — `typing_appends_at_the_caret`
282. Inserting nothing changes nothing. — `inserting_nothing_changes_nothing`
283. Typing replaces the selection. — `typing_replaces_the_selection`
284. Backspace and delete. — `backspace_and_delete`
285. Selection is deleted as a whole. — `selection_is_deleted_as_a_whole`
286. Arrows collapse a selection to its edge. — `arrows_collapse_a_selection_to_its_edge`
287. Shift arrows extend in either direction. — `shift_arrows_extend_in_either_direction`
288. Movement stops at the ends. — `movement_stops_at_the_ends`
289. Words. — `words`
290. Delete by word. — `delete_by_word`
291. Double click selects a word. — `double_click_selects_a_word`
292. Graphemes are never split. — `graphemes_are_never_split`
293. Positions past the end are clamped. — `positions_past_the_end_are_clamped`
294. Newlines become spaces. — `newlines_become_spaces`
295. Filter and length limit. — `filter_and_length_limit`
296. Typing in a row is one undo step. — `typing_in_a_row_is_one_undo_step`
297. Moving the caret ends the typing run. — `moving_the_caret_ends_the_typing_run`
298. Paste cut and delete are separate steps. — `paste_cut_and_delete_are_separate_steps`
299. New edits clear redo. — `new_edits_clear_redo`
300. Backspaces in a row coalesce. — `backspaces_in_a_row_coalesce`
301. Copy needs a selection. — `copy_needs_a_selection`
302. Replace range for input methods. — `replace_range_for_input_methods`
303. Set text resets history. — `set_text_resets_history`
304. Undo history is bounded. — `undo_history_is_bounded`
*crates/shell-core/tests/settings_run.rs*

305. Defaults match jmeters own. — `defaults_match_jmeters_own`
306. Values round trip. — `values_round_trip`
307. Startup is stored as a word. — `startup_is_stored_as_a_word`
308. Out of range values are clamped. — `out_of_range_values_are_clamped`
309. An unknown startup word counts as damage. — `an_unknown_startup_word_counts_as_damage`
310. Old files without the new fields get defaults. — `old_files_without_the_new_fields_get_defaults`

### jvm-runtime (finding Java and JMeter, running and stopping JMeter, results, live numbers, run folders)

*crates/jvm-runtime/src/live.rs*

311. Totals are exact. — `totals_are_exact`
312. Percentiles use the nearest rank. — `percentiles_use_the_nearest_rank`
313. One sample has every percentile. — `one_sample_has_every_percentile`
314. Throughput is samples over the time covered. — `throughput_is_samples_over_the_time_covered`
315. Labels are kept apart and capped. — `labels_are_kept_apart_and_capped`
316. The series groups samples by the second they finished. — `the_series_groups_samples_by_the_second_they_finished`
317. A late row goes into the step it belongs to. — `a_late_row_goes_into_the_step_it_belongs_to`
318. A long run merges steps instead of growing. — `a_long_run_merges_steps_instead_of_growing`
319. Only the latest errors are kept. — `only_the_latest_errors_are_kept`
320. Absurd values do not break anything. — `absurd_values_do_not_break_anything`
321. A finished results file is read whole. — `a_finished_results_file_is_read_whole`
322. Empty stats are all zero. — `empty_stats_are_all_zero`
323. A million samples are added fast and in bounded memory. — `a_million_samples_are_added_fast_and_in_bounded_memory`
*crates/jvm-runtime/src/locate.rs*

324. Java versions. — `java_versions`
325. A script is never used as java. — `a_script_is_never_used_as_java`
326. A bare program name is found on the path and other paths are kept. — `a_bare_program_name_is_found_on_the_path_and_other_paths_are_kept`
*crates/jvm-runtime/src/results.rs*

327. Summary interval line. — `summary_interval_line`
328. Summary cumulative line has no threads. — `summary_cumulative_line_has_no_threads`
329. Summary rate when elapsed is zero. — `summary_rate_when_elapsed_is_zero`
330. Stop port line. — `stop_port_line`
331. Jtl tail handles partial lines and quotes. — `jtl_tail_handles_partial_lines_and_quotes`
*crates/jvm-runtime/src/run.rs*

332. A process ended by a signal still has an exit code. — `a_process_ended_by_a_signal_still_has_an_exit_code`
333. The long windows form of a path is shortened. — `the_long_windows_form_of_a_path_is_shortened`
*crates/jvm-runtime/src/runs.rs*

334. Runs go in a runs folder next to the plan or inside the chosen folder. — `runs_go_in_a_runs_folder_next_to_the_plan_or_inside_the_chosen_folder`
335. A custom place needs a folder otherwise the plan folder is used. — `a_custom_place_needs_a_folder_otherwise_the_plan_folder_is_used`
336. Runs of a plan are found however the plan path is spelled. — `runs_of_a_plan_are_found_however_the_plan_path_is_spelled`
337. Preparing makes the runs folder and checks it can be written. — `preparing_makes_the_runs_folder_and_checks_it_can_be_written`
338. A missing or wrong custom folder is reported. — `a_missing_or_wrong_custom_folder_is_reported`
339. A new run gets its own folder named after the plan and time. — `a_new_run_gets_its_own_folder_named_after_the_plan_and_time`
340. Plan names that try to leave the folder stay inside it. — `plan_names_that_try_to_leave_the_folder_stay_inside_it`
341. Listing shows this apps run folders newest first and ignores the rest. — `listing_shows_this_apps_run_folders_newest_first_and_ignores_the_rest`
342. A damaged or huge run file is ignored. — `a_damaged_or_huge_run_file_is_ignored`
343. Only the newest runs of a plan are kept with all their files. — `only_the_newest_runs_of_a_plan_are_kept_with_all_their_files`
344. A run that has not ended is never pruned and keep is at least one. — `a_run_that_has_not_ended_is_never_pruned_and_keep_is_at_least_one`
345. Clearing removes only this apps run folders. — `clearing_removes_only_this_apps_run_folders`
346. Clearing refuses a folder that is not named runs. — `clearing_refuses_a_folder_that_is_not_named_runs`
347. Clearing never follows a link out of the folder. — `clearing_never_follows_a_link_out_of_the_folder`
348. Folder size adds up the files. — `folder_size_adds_up_the_files`
*crates/jvm-runtime/tests/mutations.rs*

349. Damaged results files never crash the reader. — `damaged_results_files_never_crash_the_reader`
350. A file written in pieces reads the same as the whole file. — `a_file_written_in_pieces_reads_the_same_as_the_whole_file`
351. Damaged console lines never crash the parsers. — `damaged_console_lines_never_crash_the_parsers`
*crates/jvm-runtime/tests/results_security.rs*

352. Rows are read with every column. — `rows_are_read_with_every_column`
353. Crlf line ends and quoted fields with commas quotes and line breaks are read. — `crlf_line_ends_and_quoted_fields_with_commas_quotes_and_line_breaks_are_read`
354. Rows written in pieces are found when complete. — `rows_written_in_pieces_are_found_when_complete`
355. A missing file gives no rows and no error. — `a_missing_file_gives_no_rows_and_no_error`
356. Missing columns and absurd numbers become zero not errors. — `missing_columns_and_absurd_numbers_become_zero_not_errors`
357. A record larger than the limit is dropped and the rows after it are kept. — `a_record_larger_than_the_limit_is_dropped_and_the_rows_after_it_are_kept`
358. A line that never ends does not grow memory. — `a_line_that_never_ends_does_not_grow_memory`
359. An open quote that never closes is dropped too. — `an_open_quote_that_never_closes_is_dropped_too`
360. Text that is not a results file gives no samples. — `text_that_is_not_a_results_file_gives_no_samples`
361. Binary junk is survived. — `binary_junk_is_survived`
362. A huge file is read in bounded steps. — `a_huge_file_is_read_in_bounded_steps`
363. Text with accents and emoji survives the read limit boundary. — `text_with_accents_and_emoji_survives_the_read_limit_boundary`
364. A file replaced by a shorter one is read from the start. — `a_file_replaced_by_a_shorter_one_is_read_from_the_start`
365. Half a million rows are read quickly. — `half_a_million_rows_are_read_quickly`
*crates/jvm-runtime/tests/run_jmeter.rs*

366. A normal run is accepted. — `a_normal_run_is_accepted`
367. A script is refused as the java program. — `a_script_is_refused_as_the_java_program`
368. Property names that could add options or commands are refused. — `property_names_that_could_add_options_or_commands_are_refused`
369. Property values and paths with control characters are refused. — `property_values_and_paths_with_control_characters_are_refused`
370. Only memory and define options are passed to java. — `only_memory_and_define_options_are_passed_to_java`
371. Missing pieces are named in the message. — `missing_pieces_are_named_in_the_message`
372. Starting with a refused spec starts nothing. — `starting_with_a_refused_spec_starts_nothing`
373. A real run reports every sample and ends cleanly. — `a_real_run_reports_every_sample_and_ends_cleanly`
374. Folders and plans with spaces quotes and symbols still run. — `folders_and_plans_with_spaces_quotes_and_symbols_still_run`
375. Paths with letters outside the system code page get a warning. — `paths_with_letters_outside_the_system_code_page_get_a_warning`
376. A plan whose name looks like an option is still a plan. — `a_plan_whose_name_looks_like_an_option_is_still_a_plan`
377. Stop lets a run that never ends finish by itself. — `stop_lets_a_run_that_never_ends_finish_by_itself`
378. Stop now ends a run quickly. — `stop_now_ends_a_run_quickly`
379. Kill ends a run that is not listening. — `kill_ends_a_run_that_is_not_listening`
380. Dropping the handle never leaves jmeter running. — `dropping_the_handle_never_leaves_jmeter_running`
381. The stop port answers this machine only. — `the_stop_port_answers_this_machine_only`
382. Each run gets its own stop port. — `each_run_gets_its_own_stop_port`
383. A flood of output is bounded and does not block the run. — `a_flood_of_output_is_bounded_and_does_not_block_the_run`
384. A run with a bad plan reports the failure. — `a_run_with_a_bad_plan_reports_the_failure`
385. Time from start to the first sample is measured. — `time_from_start_to_the_first_sample_is_measured`
386. Jmeter is ended when the app crashes. — `jmeter_is_ended_when_the_app_crashes`

### app (pure logic inside the window crate)

*crates/app/src/assets.rs*

387. Every category icon is bundled. — `every_category_icon_is_bundled`
388. Fonts are bundled. — `fonts_are_bundled`
389. Icons used by the ui exist. — `icons_used_by_the_ui_exist`
390. Missing assets return none. — `missing_assets_return_none`
*crates/app/src/chart.rs*

391. The scale is the largest value but at least one. — `the_scale_is_the_largest_value_but_at_least_one`
392. Points are spread across the width with zero at the bottom. — `points_are_spread_across_the_width_with_zero_at_the_bottom`
393. Values outside the scale stay inside the chart. — `values_outside_the_scale_stay_inside_the_chart`
*crates/app/src/combo.rs*

394. Selection is clamped to the options. — `selection_is_clamped_to_the_options`
*crates/app/src/palette_view.rs*

395. Empty messages. — `empty_messages`
*crates/app/src/running.rs*

396. A run starts as starting and becomes running on the first report. — `a_run_starts_as_starting_and_becomes_running_on_the_first_report`
397. Samples feed the numbers and lines the log. — `samples_feed_the_numbers_and_lines_the_log`
398. The log keeps only the latest lines. — `the_log_keeps_only_the_latest_lines`
399. How a run ends decides its outcome. — `how_a_run_ends_decides_its_outcome`
400. A failed run says why from what jmeter printed. — `a_failed_run_says_why_from_what_jmeter_printed`
401. Asking to stop moves to stopping and force is offered only after the grace time. — `asking_to_stop_moves_to_stopping_and_force_is_offered_only_after_the_grace_time`
402. Stopping something that is not running does nothing. — `stopping_something_that_is_not_running_does_nothing`
403. A stored run takes its final phase once its numbers are in. — `a_stored_run_takes_its_final_phase_once_its_numbers_are_in`
404. A stored run shows the length it had not the time since it was opened. — `a_stored_run_shows_the_length_it_had_not_the_time_since_it_was_opened`
405. Phases know whether the process is going. — `phases_know_whether_the_process_is_going`
*crates/app/src/shell.rs*

406. Shortcut conversion. — `shortcut_conversion`
407. Every shortcut is a valid keystroke. — `every_shortcut_is_a_valid_keystroke`
408. Display path strips the verbatim prefix. — `display_path_strips_the_verbatim_prefix`
409. Runtime check reports a missing java. — `runtime_check_reports_a_missing_java`
410. Runtime check reports a bad jmeter folder. — `runtime_check_reports_a_bad_jmeter_folder`
*crates/app/src/theme.rs*

411. Contrast math matches known values. — `contrast_math_matches_known_values`
412. Text is readable in both modes. — `text_is_readable_in_both_modes`
413. Colours that carry meaning stand out from the background. — `colours_that_carry_meaning_stand_out_from_the_background`
414. Dark is dark and light is light. — `dark_is_dark_and_light_is_light`
415. Surfaces are distinguishable from each other. — `surfaces_are_distinguishable_from_each_other`
416. Every category has a colour. — `every_category_has_a_colour`
417. Alpha helper only changes alpha. — `alpha_helper_only_changes_alpha`
418. Selection is translucent. — `selection_is_translucent`
*crates/app/src/ui_util.rs*

419. Highlight groups adjacent characters. — `highlight_groups_adjacent_characters`
420. Highlight without positions is one run. — `highlight_without_positions_is_one_run`
421. Highlight counts characters not bytes. — `highlight_counts_characters_not_bytes`
*crates/app/src/views_content.rs*

422. Java version is shortened. — `java_version_is_shortened`
*crates/app/src/views_run.rs*

423. The clock shows minutes and seconds and hours when needed. — `the_clock_shows_minutes_and_seconds_and_hours_when_needed`
424. Counts get thousands separators. — `counts_get_thousands_separators`
425. Rates are short. — `rates_are_short`

## 2. Whole-window tests (headless)

These run the real window without a screen, driving it with simulated keys, clicks and drags.

### Opening plans, tree, palette, tabs, panels, themes, settings

*crates/app/tests/shell.rs*

426. Starts empty without a plan. — `starts_empty_without_a_plan`
427. Opens a plan given at startup. — `opens_a_plan_given_at_startup`
428. A missing file shows an error and leaves the app usable. — `a_missing_file_shows_an_error_and_leaves_the_app_usable`
429. A file that is not a plan shows an error. — `a_file_that_is_not_a_plan_shows_an_error`
430. A hostile plan is refused with a message and no tab. — `a_hostile_plan_is_refused_with_a_message_and_no_tab`
431. Opening adds to recent files and failures remove them. — `opening_adds_to_recent_files_and_failures_remove_them`
432. Clicking a row redraws the editor and opens no tab. — `clicking_a_row_redraws_the_editor_and_opens_no_tab`
433. Every kind of element redraws the same tab. — `every_kind_of_element_redraws_the_same_tab`
434. Keyboard navigation in the tree. — `keyboard_navigation_in_the_tree`
435. Enter and space expand and collapse the selected element. — `enter_and_space_expand_and_collapse_the_selected_element`
436. Left and right collapse and expand. — `left_and_right_collapse_and_expand`
437. The chevron toggles without selecting. — `the_chevron_toggles_without_selecting`
438. Expand and collapse all from the palette. — `expand_and_collapse_all_from_the_palette`
439. Ctrl k opens and escape closes the palette. — `ctrl_k_opens_and_escape_closes_the_palette`
440. Clicking outside closes the palette. — `clicking_outside_closes_the_palette`
441. The title bar search box opens the palette. — `the_title_bar_search_box_opens_the_palette`
442. Palette filters as you type. — `palette_filters_as_you_type`
443. Goto element selects opens and focuses the tree. — `goto_element_selects_opens_and_focuses_the_tree`
444. Goto expands collapsed ancestors. — `goto_expands_collapsed_ancestors`
445. Arrow keys move through palette results. — `arrow_keys_move_through_palette_results`
446. Unavailable commands do nothing when chosen. — `unavailable_commands_do_nothing_when_chosen`
447. Add mode lists what fits under the selection. — `add_mode_lists_what_fits_under_the_selection`
448. The split editor is gone. — `the_split_editor_is_gone`
449. Opening the same plan again shows its tab. — `opening_the_same_plan_again_shows_its_tab`
450. Each plan has one tab and its own tree. — `each_plan_has_one_tab_and_its_own_tree`
451. Closing a plan tab closes the plan. — `closing_a_plan_tab_closes_the_plan`
452. Middle click closes a tab. — `middle_click_closes_a_tab`
453. Ctrl tab cycles through plans. — `ctrl_tab_cycles_through_plans`
454. Selecting an element brings the plan back from settings. — `selecting_an_element_brings_the_plan_back_from_settings`
455. The activity bar has no search item. — `the_activity_bar_has_no_search_item`
456. The inspector starts hidden and shows what can be added. — `the_inspector_starts_hidden_and_shows_what_can_be_added`
457. Can add follows the selection. — `can_add_follows_the_selection`
458. Panel shortcuts toggle and persist. — `panel_shortcuts_toggle_and_persist`
459. Dragging the sidebar handle resizes and saves. — `dragging_the_sidebar_handle_resizes_and_saves`
460. Theme commands switch and persist. — `theme_commands_switch_and_persist`
461. Settings tab opens and saves runtime paths. — `settings_tab_opens_and_saves_runtime_paths`
462. Settings tab theme buttons. — `settings_tab_theme_buttons`
463. About dialog opens from the palette and closes on click. — `about_dialog_opens_from_the_palette_and_closes_on_click`
464. Notices disappear by themselves. — `notices_disappear_by_themselves`
465. Reload keeps the selection. — `reload_keeps_the_selection`
466. Reload after the file is deleted shows an error and keeps the old plan. — `reload_after_the_file_is_deleted_shows_an_error_and_keeps_the_old_plan`
467. Recent plans appear in the palette and reopen. — `recent_plans_appear_in_the_palette_and_reopen`
468. A damaged settings file is reported at startup. — `a_damaged_settings_file_is_reported_at_startup`
469. Problems panel lists plan notes. — `problems_panel_lists_plan_notes`
470. Valid heap and interval are saved. — `valid_heap_and_interval_are_saved`
471. Invalid numbers are not saved and are reported. — `invalid_numbers_are_not_saved_and_are_reported`
472. A variable is not accepted for heap. — `a_variable_is_not_accepted_for_heap`
473. Enter in a number field applies. — `enter_in_a_number_field_applies`
474. The startup combo saves its choice. — `the_startup_combo_saves_its_choice`
475. Startup can reopen the last plan. — `startup_can_reopen_the_last_plan`
476. Startup skips a last plan that no longer exists. — `startup_skips_a_last_plan_that_no_longer_exists`
477. A plan on the command line wins over the startup setting. — `a_plan_on_the_command_line_wins_over_the_startup_setting`
478. Welcome screen stays when startup is welcome. — `welcome_screen_stays_when_startup_is_welcome`
479. The state file is never seen half written. — `the_state_file_is_never_seen_half_written`

### Editing: add, delete, move, drag and drop, undo, saving, recovery, scope, big plans

*crates/app/tests/editing.rs*

480. Adding from the palette puts the element under the selection and selects it. — `adding_from_the_palette_puts_the_element_under_the_selection_and_selects_it`
481. Adding with nothing selected goes under the test plan. — `adding_with_nothing_selected_goes_under_the_test_plan`
482. Clicking a name in can add adds it at once. — `clicking_a_name_in_can_add_adds_it_at_once`
483. Only allowed types can be added under the test plan. — `only_allowed_types_can_be_added_under_the_test_plan`
484. Delete removes the element selects a neighbour and undo brings it back. — `delete_removes_the_element_selects_a_neighbour_and_undo_brings_it_back`
485. The test plan cannot be deleted. — `the_test_plan_cannot_be_deleted`
486. Duplicate copies the element with its children. — `duplicate_copies_the_element_with_its_children`
487. Alt arrows move an element among its siblings. — `alt_arrows_move_an_element_among_its_siblings`
488. Ctrl t switches an element off and on. — `ctrl_t_switches_an_element_off_and_on`
489. Undo and redo walk through the history. — `undo_and_redo_walk_through_the_history`
490. A new edit after undo clears redo. — `a_new_edit_after_undo_clears_redo`
491. The edit commands are in the palette with their shortcuts. — `the_edit_commands_are_in_the_palette_with_their_shortcuts`
492. Ctrl s saves the edits to the file and clears the unsaved mark. — `ctrl_s_saves_the_edits_to_the_file_and_clears_the_unsaved_mark`
493. The recovery copy is written after a pause and removed on save. — `the_recovery_copy_is_written_after_a_pause_and_removed_on_save`
494. Only the latest edit writes the recovery copy. — `only_the_latest_edit_writes_the_recovery_copy`
495. Undoing back to the saved state removes the recovery copy. — `undoing_back_to_the_saved_state_removes_the_recovery_copy`
496. A recovery copy from an earlier session can be recovered. — `a_recovery_copy_from_an_earlier_session_can_be_recovered`
497. The recovery copy can be discarded. — `the_recovery_copy_can_be_discarded`
498. A copy that matches the file is ignored. — `a_copy_that_matches_the_file_is_ignored`
499. Closing a changed plan asks and cancel keeps it. — `closing_a_changed_plan_asks_and_cancel_keeps_it`
500. Dont save closes and drops the recovery copy. — `dont_save_closes_and_drops_the_recovery_copy`
501. Save in the question saves then closes. — `save_in_the_question_saves_then_closes`
502. Closing an unchanged plan does not ask. — `closing_an_unchanged_plan_does_not_ask`
503. Quitting with unsaved changes asks first. — `quitting_with_unsaved_changes_asks_first`
504. Close all asks once for every changed plan. — `close_all_asks_once_for_every_changed_plan`
505. Reloading a changed plan asks first. — `reloading_a_changed_plan_asks_first`
506. Each plan has its own history and unsaved mark. — `each_plan_has_its_own_history_and_unsaved_mark`
507. Save as writes a new file and renames the tab. — `save_as_writes_a_new_file_and_renames_the_tab`
508. Edits keep the selection and the tree in step. — `edits_keep_the_selection_and_the_tree_in_step`
509. Dropping onto an element makes it the last child. — `dropping_onto_an_element_makes_it_the_last_child`
510. Dropping above or below a row reorders. — `dropping_above_or_below_a_row_reorders`
511. Dropping below the last sibling moves to the end. — `dropping_below_the_last_sibling_moves_to_the_end`
512. A move the rules forbid is refused with a message. — `a_move_the_rules_forbid_is_refused_with_a_message`
513. The test plan cannot be dragged and nothing sits beside it. — `the_test_plan_cannot_be_dragged_and_nothing_sits_beside_it`
514. An element cannot be dropped into its own subtree. — `an_element_cannot_be_dropped_into_its_own_subtree`
515. The inspector lists what applies to a sampler and clicking selects it. — `the_inspector_lists_what_applies_to_a_sampler_and_clicking_selects_it`
516. The inspector shows what a config element applies to. — `the_inspector_shows_what_a_config_element_applies_to`
517. Scope follows edits. — `scope_follows_edits`
518. A plan of twenty thousand elements stays usable. — `a_plan_of_twenty_thousand_elements_stays_usable`

### Forms: boxes, numbers, drop-downs, tables, typing, hints, focus

*crates/app/tests/forms.rs*

519. A typed value is saved with enter and undone with undo. — `a_typed_value_is_saved_with_enter_and_undone_with_undo`
520. Leaving a box saves what was typed. — `leaving_a_box_saves_what_was_typed`
521. Tab moves to the next box and saves the one left. — `tab_moves_to_the_next_box_and_saves_the_one_left`
522. Escape puts the saved value back. — `escape_puts_the_saved_value_back`
523. The name box renames the element in the tree. — `the_name_box_renames_the_element_in_the_tree`
524. F2 puts the cursor in the name box. — `f2_puts_the_cursor_in_the_name_box`
525. The comment box edits the comments. — `the_comment_box_edits_the_comments`
526. Ctrl s saves text that was typed but not confirmed. — `ctrl_s_saves_text_that_was_typed_but_not_confirmed`
527. Number fields accept numbers and variables but not letters. — `number_fields_accept_numbers_and_variables_but_not_letters`
528. A variable in a number setting turns it into a string setting. — `a_variable_in_a_number_setting_turns_it_into_a_string_setting`
529. The loop count lives inside the main controller setting. — `the_loop_count_lives_inside_the_main_controller_setting`
530. Checkboxes toggle a setting and undo reverts. — `checkboxes_toggle_a_setting_and_undo_reverts`
531. A drop down saves the chosen value. — `a_drop_down_saves_the_chosen_value`
532. Undo updates the drop down. — `undo_updates_the_drop_down`
533. A value jmeter does not list is kept as a custom choice. — `a_value_jmeter_does_not_list_is_kept_as_a_custom_choice`
534. A setting made of several flags is edited flag by flag. — `a_setting_made_of_several_flags_is_edited_flag_by_flag`
535. Parameters are added edited and removed in a table. — `parameters_are_added_edited_and_removed_in_a_table`
536. Table changes undo in order. — `table_changes_undo_in_order`
537. User defined variables have their own table. — `user_defined_variables_have_their_own_table`
538. Assertion patterns are a list of strings. — `assertion_patterns_are_a_list_of_strings`
539. Elements without a hand made form get one from their settings. — `elements_without_a_hand_made_form_get_one_from_their_settings`
540. Long text is shown but not edited. — `long_text_is_shown_but_not_edited`
541. The test plan has a variables table and flags. — `the_test_plan_has_a_variables_table_and_flags`
542. Enabled button switches the element off. — `enabled_button_switches_the_element_off`
543. The form follows the selection. — `the_form_follows_the_selection`
544. A variable nothing defines shows up in the problems list. — `a_variable_nothing_defines_shows_up_in_the_problems_list`
545. The variable hints stay up to date with undo. — `the_variable_hints_stay_up_to_date_with_undo`
546. The box keeps the focus after enter even when the form changes shape. — `the_box_keeps_the_focus_after_enter_even_when_the_form_changes_shape`
547. Shortcuts work right after editing a box. — `shortcuts_work_right_after_editing_a_box`
548. Undo from a box keeps the window usable. — `undo_from_a_box_keeps_the_window_usable`
549. Typing marks the plan unsaved at once and deleting it again clears the mark. — `typing_marks_the_plan_unsaved_at_once_and_deleting_it_again_clears_the_mark`
550. Closing a tab asks about text that was only typed. — `closing_a_tab_asks_about_text_that_was_only_typed`
551. Typing in a table cell marks the plan at once. — `typing_in_a_table_cell_marks_the_plan_at_once`
552. Text typed in a box reaches the plan and the recovery copy after a pause. — `text_typed_in_a_box_reaches_the_plan_and_the_recovery_copy_after_a_pause`
553. Text typed in a table cell reaches the recovery copy. — `text_typed_in_a_table_cell_reaches_the_recovery_copy`
554. Half a number does not raise an error while typing. — `half_a_number_does_not_raise_an_error_while_typing`
555. The name box renames while typing. — `the_name_box_renames_while_typing`

### Text and number boxes

*crates/app/tests/fields.rs*

556. Typing and backspace. — `typing_and_backspace`
557. Caret movement and insertion. — `caret_movement_and_insertion`
558. Select all and replace. — `select_all_and_replace`
559. Shift arrows select and delete. — `shift_arrows_select_and_delete`
560. Word navigation and deletion. — `word_navigation_and_deletion`
561. Undo and redo. — `undo_and_redo`
562. Clipboard copy cut paste. — `clipboard_copy_cut_paste`
563. Pasting multiple lines gives one line. — `pasting_multiple_lines_gives_one_line`
564. Enter and escape emit events. — `enter_and_escape_emit_events`
565. Changed is emitted for every edit but not for navigation. — `changed_is_emitted_for_every_edit_but_not_for_navigation`
566. Number field rejects letters. — `number_field_rejects_letters`
567. Number field steps with arrow keys. — `number_field_steps_with_arrow_keys`
568. Number field accepts variables and does not step them. — `number_field_accepts_variables_and_does_not_step_them`
569. Number field reports problems. — `number_field_reports_problems`
570. Read only field ignores edits but allows copy. — `read_only_field_ignores_edits_but_allows_copy`
571. Set text replaces the content. — `set_text_replaces_the_content`
572. Typing unicode. — `typing_unicode`
573. Losing the focus is reported once. — `losing_the_focus_is_reported_once`

### Running plans: questions before a run, live numbers, stopping, run history, the runs folder, settings

*crates/app/tests/runs.rs*

574. Without java or jmeter the run buttons say why. — `without_java_or_jmeter_the_run_buttons_say_why`
575. The run menu and palette list every run command. — `the_run_menu_and_palette_list_every_run_command`
576. A plan with unsaved changes asks to save before running. — `a_plan_with_unsaved_changes_asks_to_save_before_running`
577. Saving to run writes the file then goes on to the next question. — `saving_to_run_writes_the_file_then_goes_on_to_the_next_question`
578. A runs folder that cannot be used stops the run and offers a way out. — `a_runs_folder_that_cannot_be_used_stops_the_run_and_offers_a_way_out`
579. Using the jmx folder from the question changes the setting and carries on. — `using_the_jmx_folder_from_the_question_changes_the_setting_and_carries_on`
580. A custom folder that is a file is refused. — `a_custom_folder_that_is_a_file_is_refused`
581. A plan with scripts asks once for that plan file. — `a_plan_with_scripts_asks_once_for_that_plan_file`
582. A plan without scripts is never asked about them. — `a_plan_without_scripts_is_never_asked_about_them`
583. The settings tab chooses where run files go. — `the_settings_tab_chooses_where_run_files_go`
584. The number of runs kept is a checked number. — `the_number_of_runs_kept_is_a_checked_number`
585. Lowering the number kept removes the older runs of the open plan at once. — `lowering_the_number_kept_removes_the_older_runs_of_the_open_plan_at_once`
586. The history tab lists the runs of the open plan newest first. — `the_history_tab_lists_the_runs_of_the_open_plan_newest_first`
587. History shows runs of the plan in the tab not every plan. — `history_shows_runs_of_the_plan_in_the_tab_not_every_plan`
588. Clicking a stored run shows its numbers and charts. — `clicking_a_stored_run_shows_its_numbers_and_charts`
589. A damaged results file still opens as a stored run. — `a_damaged_results_file_still_opens_as_a_stored_run`
590. A run that was cut short is shown as interrupted. — `a_run_that_was_cut_short_is_shown_as_interrupted`
591. Removing one run asks first and removes only that run. — `removing_one_run_asks_first_and_removes_only_that_run`
592. Removing never touches a folder that is not a run. — `removing_never_touches_a_folder_that_is_not_a_run`
593. Clearing the shared runs folder asks and removes only runs. — `clearing_the_shared_runs_folder_asks_and_removes_only_runs`
594. Clear is not available for the runs next to the plan. — `clear_is_not_available_for_the_runs_next_to_the_plan`
595. Clearing an empty shared folder says so without asking. — `clearing_an_empty_shared_folder_says_so_without_asking`
596. F5 runs the saved plan and shows the live numbers. — `f5_runs_the_saved_plan_and_shows_the_live_numbers`
597. The run header says where the files are and the status bar follows the run. — `the_run_header_says_where_the_files_are_and_the_status_bar_follows_the_run`
598. Stop ends a run that would never end and it is kept as stopped. — `stop_ends_a_run_that_would_never_end_and_it_is_kept_as_stopped`
599. Stop now and ending jmeter by force both work. — `stop_now_and_ending_jmeter_by_force_both_work`
600. The chosen folder gets a runs folder with a folder for each run. — `the_chosen_folder_gets_a_runs_folder_with_a_folder_for_each_run`
601. Only the newest runs are kept after a run and their files stay. — `only_the_newest_runs_are_kept_after_a_run_and_their_files_stay`
602. Validate runs the whole plan once with one user. — `validate_runs_the_whole_plan_once_with_one_user`
603. Send once sends the selected request and only that. — `send_once_sends_the_selected_request_and_only_that`
604. Send once is not available on something that is not a request. — `send_once_is_not_available_on_something_that_is_not_a_request`
605. Quitting during a run asks and cancelling leaves the run going. — `quitting_during_a_run_asks_and_cancelling_leaves_the_run_going`
606. A plan jmeter cannot read ends as failed with a reason. — `a_plan_jmeter_cannot_read_ends_as_failed_with_a_reason`
607. A run with a flood of samples keeps the window responsive. — `a_run_with_a_flood_of_samples_keeps_the_window_responsive`
608. Viewing a large stored run is quick and does not block the window. — `viewing_a_large_stored_run_is_quick_and_does_not_block_the_window`

## 3. Real-window checks

These start the built app, drive it with real mouse and keyboard input, and read its state file and the files it writes (`tests/ui/smoke.ps1`).


### 1. Open a plan, select by mouse, navigate by keyboard

609. Plan loads. — `smoke.ps1`, section "1. Open a plan, select by mouse, navigate by keyboard"
610. Tree shows 15 rows. — `smoke.ps1`, section "1. Open a plan, select by mouse, navigate by keyboard"
611. Clicking a row selects it. — `smoke.ps1`, section "1. Open a plan, select by mouse, navigate by keyboard"
612. And the editor shows it. — `smoke.ps1`, section "1. Open a plan, select by mouse, navigate by keyboard"
613. Without opening a tab. — `smoke.ps1`, section "1. Open a plan, select by mouse, navigate by keyboard"
614. Down moves the selection and the editor follows. — `smoke.ps1`, section "1. Open a plan, select by mouse, navigate by keyboard"
615. End selects the last row. — `smoke.ps1`, section "1. Open a plan, select by mouse, navigate by keyboard"

### 2. Command palette

616. Ctrl+K opens the palette. — `smoke.ps1`, section "2. Command palette"
617. Typing filters commands. — `smoke.ps1`, section "2. Command palette"
618. Escape closes it. — `smoke.ps1`, section "2. Command palette"
619. Go to element selects it. — `smoke.ps1`, section "2. Command palette"
620. And the editor shows it, still in one tab. — `smoke.ps1`, section "2. Command palette"

### 3. Inspector

621. The inspector starts hidden. — `smoke.ps1`, section "3. Inspector"
622. Ctrl+Alt+B shows it. — `smoke.ps1`, section "3. Inspector"
623. An empty title bar click changes nothing. — `smoke.ps1`, section "3. Inspector"

### 4. Menus

624. Ctrl+J shows the results panel. — `smoke.ps1`, section "4. Menus"
625. Ctrl+B hides the test plan panel. — `smoke.ps1`, section "4. Menus"

### 5. Themes

626. Light theme applies. — `smoke.ps1`, section "5. Themes"
627. Dark theme applies. — `smoke.ps1`, section "5. Themes"

### 6. Resizing a panel and remembering it

628. Dragging the handle widens the panel. — `smoke.ps1`, section "6. Resizing a panel and remembering it"

### 7. Window controls and remembered size

629. Maximize fills the screen. — `smoke.ps1`, section "7. Window controls and remembered size"
630. Close button ends the app. — `smoke.ps1`, section "7. Window controls and remembered size"
631. Window state is saved. — `smoke.ps1`, section "7. Window controls and remembered size"

### 8. Settings tab and drop-down

632. Ctrl+, opens settings. — `smoke.ps1`, section "8. Settings tab and drop-down"
633. Choosing from the drop-down with the keyboard saves it. — `smoke.ps1`, section "8. Settings tab and drop-down"
634. Choosing with the mouse saves it. — `smoke.ps1`, section "8. Settings tab and drop-down"
635. An invalid heap size is rejected. — `smoke.ps1`, section "8. Settings tab and drop-down"
636. A valid heap size is saved. — `smoke.ps1`, section "8. Settings tab and drop-down"

### 9. Open dialog

637. Ctrl+O opens a file dialog and loads the chosen plan. — `smoke.ps1`, section "9. Open dialog"
638. A second plan gets its own tab. — `smoke.ps1`, section "9. Open dialog"
639. And the tree shows the second plan. — `smoke.ps1`, section "9. Open dialog"
640. Ctrl+Tab switches back to the first plan. — `smoke.ps1`, section "9. Open dialog"

### 10. Editing: forms, add, delete, undo, drag, save, recovery

641. Go to element shows its form. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
642. Typing in a box and pressing Enter changes the plan. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
643. Clicking a checkbox is another change. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
644. Ctrl+Z undoes the last change. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
645. Ctrl+S saves. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
646. The file keeps its original formatting elsewhere. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
647. Adding from the palette selects the new element. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
648. Delete removes the selected element. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
649. And undo brings it back. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
650. Dragging a row onto another moves the element. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
651. A recovery copy is written next to the plan after a pause. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
652. Opening the plan again offers the unsaved work. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
653. Recovering brings the changes back, unsaved. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
654. Closing the window with unsaved changes asks first. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
655. Cancel keeps the app open with the work. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
656. Quitting with unsaved changes asks first. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
657. Don't Save quits. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
658. And removes the recovery copy. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"

### 11. Running: F5, live numbers, history, Validate, Send once

659. Java and JMeter are found in the background. — `smoke.ps1`, section "11. Running: F5, live numbers, history, Validate, Send once"
660. F5 starts the run and opens the lower panel on the Run tab. — `smoke.ps1`, section "11. Running: F5, live numbers, history, Validate, Send once"
661. The run finishes. — `smoke.ps1`, section "11. Running: F5, live numbers, history, Validate, Send once"
662. All 12 samples were counted, 6 of them errors. — `smoke.ps1`, section "11. Running: F5, live numbers, history, Validate, Send once"
663. The plan file was not changed by the run. — `smoke.ps1`, section "11. Running: F5, live numbers, history, Validate, Send once"
664. The run has its own folder inside runs next to the plan. — `smoke.ps1`, section "11. Running: F5, live numbers, history, Validate, Send once"
665. With results, log, the plan as run and run.json. — `smoke.ps1`, section "11. Running: F5, live numbers, history, Validate, Send once"
666. The history lists the run. — `smoke.ps1`, section "11. Running: F5, live numbers, history, Validate, Send once"
667. Ctrl+F5 validates: one user once, so each request once. — `smoke.ps1`, section "11. Running: F5, live numbers, history, Validate, Send once"
668. F6 sends the selected request once. — `smoke.ps1`, section "11. Running: F5, live numbers, history, Validate, Send once"
669. All three runs are in the history. — `smoke.ps1`, section "11. Running: F5, live numbers, history, Validate, Send once"

Total scenarios listed: 669.
