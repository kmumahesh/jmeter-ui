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
- **New plans start right.** File > New Plan (Ctrl+N) offers an API test, a web site test and an empty plan; each is saved where the user chooses, opens clean, is named after the file, can be edited, saved and undone, and the two starters run against a server with every status check passing (and failing when the server fails). Odd file names, existing files, folders, plans already open and unwritable places are handled with a message.
- **Bodies and scripts are edited as text.** A text area keeps lines, indents and outdents, moves by line and page, scrolls with the caret and the wheel, selects by keyboard and mouse, undoes by steps, colours JSON, XML and `${variables}`, drops characters a plan cannot hold, and stays quick on very long texts. An HTTP request shows the body editor while it sends a body, a typed body reaches the server exactly as typed (also with a variable), and a comment or table value with several lines is never flattened by looking at it.
- **Elements can be copied, cut and pasted.** Ctrl+C, X and V work on the selected element and everything under it, inside a plan and between plans; a paste goes where JMeter's rules allow (under the selection, next to it, or higher up), is one undo step with exact undo, and never changes anything when it is refused. In a text box the same keys work on text. Text that is not elements, hostile text (entity tricks, absurd nesting, huge text) and elements that fit nowhere are refused with a message. A selection can be saved as its own file and merged into a plan, a pasted request runs, and a copy of every element of every sample plan matches its original and undoes byte for byte.
- **cURL commands become requests.** Commands from a browser, a terminal or the Windows command prompt are read (quotes, `$'...'` strings, continuations, `^` escapes, several commands), turned into requests with their headers, JSON or form bodies, multipart fields and credentials, and added to the plan in one undo step; they reach a real server exactly as the command described. Addresses that are not http or https, shell tricks, files named in a command, huge input and text with no command are refused or kept as text, never run or read, and a problem stays in the dialog.
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
- On GitHub (`.github/workflows/ci.yml`), every push and pull request installs Java 17 and JMeter 5.6.3 (download checked against Apache's SHA-512) so the tests that run the real JMeter are not skipped. The real-window checks run in their own workflow, `.github/workflows/ui-smoke.yml` (every pull request and push to `main`): the runner screen is raised to 1920x1080 first, and all 83 checks pass there in about 6.5 minutes.
- Linux and macOS: the same automated tests run there in CI (`Build and test (ubuntu-latest)`, `Build and test (macos-latest)`), including the ones that start the real JMeter. Text shortcuts in the tests use `secondary(...)` in `crates/app/tests/common/mod.rs`, which turns Ctrl into Cmd on macOS. The check that JMeter is ended when the app crashes asserts on Windows only.
- Fuzzing (Linux, nightly compiler, about a minute per target): `cargo install cargo-fuzz`, then `cargo +nightly fuzz run jmx_parse` (also `jmx_text`, `plan_load`, `results_jtl`, `curl_import`, `paste_text`, `details_file`, `har_import`) from the repo root; it runs on GitHub for changes to the readers. Test coverage: `.github/workflows/coverage.yml` (every push to `main`) keeps a summary, an lcov file and an HTML report.
- The rule that skips the build and tests for documentation-only changes on GitHub: `bash tests/ci/classify-test.sh`.
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
23. The top of a plan is its test plan and its children are reachable. — `the_top_of_a_plan_is_its_test_plan_and_its_children_are_reachable`
24. Copied elements make a plan file that holds only them. — `copied_elements_make_a_plan_file_that_holds_only_them`
25. Text with lines and markup survives being copied. — `text_with_lines_and_markup_survives_being_copied`
26. A pair from another document can be put under an element and gets new ids. — `a_pair_from_another_document_can_be_put_under_an_element_and_gets_new_ids`
27. Nothing copied is an empty plan file. — `nothing_copied_is_an_empty_plan_file`
28. A document without a plan tree has no top elements. — `a_document_without_a_plan_tree_has_no_top_elements`
*crates/jmx-model/src/plan.rs*

29. Reads simple collection and nested props. — `reads_simple_collection_and_nested_props`
*crates/jmx-model/src/xml.rs*

30. A byte order mark is kept and does not confuse the reader. — `a_byte_order_mark_is_kept_and_does_not_confuse_the_reader`
31. What the fuzzer found does not panic. — `what_the_fuzzer_found_does_not_panic`
32. A slice that would split a character is an error not a panic. — `a_slice_that_would_split_a_character_is_an_error_not_a_panic`
*crates/jmx-model/tests/mutations.rs*

33. Damaged plans never crash the reader and what it accepts is saved back exactly. — `damaged_plans_never_crash_the_reader_and_what_it_accepts_is_saved_back_exactly`
34. Damaged fragments and text never crash and escaping round trips. — `damaged_fragments_and_text_never_crash_and_escaping_round_trips`
*crates/jmx-model/tests/runcopy.rs*

35. Injects one collector under the test plan. — `injects_one_collector_under_the_test_plan`
36. The path is escaped. — `the_path_is_escaped`
37. Everything before the insertion point is unchanged. — `everything_before_the_insertion_point_is_unchanged`
38. Rejects documents without a test plan. — `rejects_documents_without_a_test_plan`
39. Sets loops everywhere and counts. — `sets_loops_everywhere_and_counts`
40. A listener with a script goes last under the test plan and the script comes back exactly. — `a_listener_with_a_script_goes_last_under_the_test_plan_and_the_script_comes_back_exactly`
41. A listener cannot be added to a document without a test plan. — `a_listener_cannot_be_added_to_a_document_without_a_test_plan`
*crates/jmx-model/tests/security.rs*

42. A doctype with an external entity is kept as text and never read. — `a_doctype_with_an_external_entity_is_kept_as_text_and_never_read`
43. An entity that points at a web address is never fetched. — `an_entity_that_points_at_a_web_address_is_never_fetched`
44. Entity expansion bombs do not grow. — `entity_expansion_bombs_do_not_grow`
45. Character references never make invalid text. — `character_references_never_make_invalid_text`
46. Deep nesting is refused with an error. — `deep_nesting_is_refused_with_an_error`
47. Nesting deeper than any real plan still opens. — `nesting_deeper_than_any_real_plan_still_opens`
48. A very wide plan opens quickly. — `a_very_wide_plan_opens_quickly`
49. A very long text value round trips. — `a_very_long_text_value_round_trips`
50. Truncating a plan at any point gives an error or a plan never a panic. — `truncating_a_plan_at_any_point_gives_an_error_or_a_plan_never_a_panic`
51. Damaged documents are errors. — `damaged_documents_are_errors`
52. Mutated plans never panic and always write back what was read. — `mutated_plans_never_panic_and_always_write_back_what_was_read`
53. Hostile text in an element stays text. — `hostile_text_in_an_element_stays_text`
54. Hostile text in an attribute stays in that attribute. — `hostile_text_in_an_attribute_stays_in_that_attribute`
55. Escaping never leaves a character that can end the value. — `escaping_never_leaves_a_character_that_can_end_the_value`
56. Characters xml cannot hold are never written. — `characters_xml_cannot_hold_are_never_written`
*crates/jmx-model/tests/xml.rs*

57. Unchanged documents are written back exactly. — `unchanged_documents_are_written_back_exactly`
58. Attributes are decoded on read. — `attributes_are_decoded_on_read`
59. Editing an attribute rewrites only that start tag. — `editing_an_attribute_rewrites_only_that_start_tag`
60. Adding a new attribute. — `adding_a_new_attribute`
61. Set text escapes and expands self closing elements. — `set_text_escapes_and_expands_self_closing_elements`
62. Text reading joins runs and decodes. — `text_reading_joins_runs_and_decodes`
63. Pushing children into a self closing element. — `pushing_children_into_a_self_closing_element`
64. Walking visits every element depth first. — `walking_visits_every_element_depth_first`
65. Elements skips text and comments. — `elements_skips_text_and_comments`
66. Malformed input is an error not a panic. — `malformed_input_is_an_error_not_a_panic`
67. Escape and unescape are inverse. — `escape_and_unescape_are_inverse`
68. Unknown entities are left alone. — `unknown_entities_are_left_alone`
69. Deeply nested documents work. — `deeply_nested_documents_work`
70. Fragments parse to one element. — `fragments_parse_to_one_element`

### plan-core (elements, rules, editing, forms, scope, variables, saving, scripts, plans made for Validate and Send once)

*crates/plan-core/src/catalog.rs*

71. Table is sorted and unique. — `table_is_sorted_and_unique`
72. Core elements. — `core_elements`
73. Shared tags use the gui class. — `shared_tags_use_the_gui_class`
74. Plugin classes are guessed from their names. — `plugin_classes_are_guessed_from_their_names`
75. Unknown core tags get a readable name. — `unknown_core_tags_get_a_readable_name`
76. Humanize handles acronyms. — `humanize_handles_acronyms`
77. Every category has an icon and labels. — `every_category_has_an_icon_and_labels`
78. List is complete and unique. — `list_is_complete_and_unique`
79. Variants are expanded. — `variants_are_expanded`
80. Every quick add entry exists. — `every_quick_add_entry_exists`
81. Sorted by category then name. — `sorted_by_category_then_name`
82. Elements missing from the table are found through jmeters menus. — `elements_missing_from_the_table_are_found_through_jmeters_menus`
83. Every menu element is classified the same way by both sources. — `every_menu_element_is_classified_the_same_way_by_both_sources`
*crates/plan-core/src/curl.rs*

84. A plain get. — `a_plain_get`
85. Url forms. — `url_forms`
86. Only http and https are read. — `only_http_and_https_are_read`
87. A json post from a browser. — `a_json_post_from_a_browser`
88. A body with lines in the browsers c style quotes. — `a_body_with_lines_in_the_browsers_c_style_quotes`
89. A form post becomes parameters not a body. — `a_form_post_becomes_parameters_not_a_body`
90. Data that is not name value pairs is a body. — `data_that_is_not_name_value_pairs_is_a_body`
91. Methods. — `methods`
92. Options that take values do not become the address. — `options_that_take_values_do_not_become_the_address`
93. Headers jmeter sets itself are left out and said so. — `headers_jmeter_sets_itself_are_left_out_and_said_so`
94. Credentials become a basic authorization header with a warning. — `credentials_become_a_basic_authorization_header_with_a_warning`
95. Cookies agents and referers are headers. — `cookies_agents_and_referers_are_headers`
96. Multipart forms. — `multipart_forms`
97. Data from a file is never read. — `data_from_a_file_is_never_read`
98. Shell tricks are text and nothing runs. — `shell_tricks_are_text_and_nothing_runs`
99. Quotes and escapes. — `quotes_and_escapes`
100. Line continuations of every shell. — `line_continuations_of_every_shell`
101. The windows command prompt form of the browsers. — `the_windows_command_prompt_form_of_the_browsers`
102. Several commands make several requests. — `several_commands_make_several_requests`
103. A command without the word curl is read as its arguments. — `a_command_without_the_word_curl_is_read_as_its_arguments`
104. Too much input and too many commands are refused. — `too_much_input_and_too_many_commands_are_refused`
105. Nothing makes it panic. — `nothing_makes_it_panic`
106. A json post becomes a request with a body and a header manager. — `a_json_post_becomes_a_request_with_a_body_and_a_header_manager`
107. A form becomes parameter rows and a plain get has no header manager. — `a_form_becomes_parameter_rows_and_a_plain_get_has_no_header_manager`
108. Markup and odd text in a command stays text in the plan. — `markup_and_odd_text_in_a_command_stays_text_in_the_plan`
109. Many requests in one import are one paste. — `many_requests_in_one_import_are_one_paste`
*crates/plan-core/src/derive.rs*

110. Validate limits every group and drops timers and backend listeners. — `validate_limits_every_group_and_drops_timers_and_backend_listeners`
111. Validate keeps every group and every sampler. — `validate_keeps_every_group_and_every_sampler`
112. Validate needs a thread group. — `validate_needs_a_thread_group`
113. Send once keeps the request and what applies to it in jmeters order. — `send_once_keeps_the_request_and_what_applies_to_it_in_jmeters_order`
114. Send once can send a request with nothing else applying. — `send_once_can_send_a_request_with_nothing_else_applying`
115. Send once refuses what is not a plain request. — `send_once_refuses_what_is_not_a_plain_request`
116. Send once skips requests under a test fragment. — `send_once_skips_requests_under_a_test_fragment`
117. Relative file names become absolute and others are left alone. — `relative_file_names_become_absolute_and_others_are_left_alone`
118. A run copy is the saved text with only the file names made absolute. — `a_run_copy_is_the_saved_text_with_only_the_file_names_made_absolute`
119. Hostile text in names and files survives the derived plan. — `hostile_text_in_names_and_files_survives_the_derived_plan`
*crates/plan-core/src/edit.rs*

120. A fresh plan is clean and writes back unchanged. — `a_fresh_plan_is_clean_and_writes_back_unchanged`
121. Adding puts the element last and gives it jmeters defaults. — `adding_puts_the_element_last_and_gives_it_jmeters_defaults`
122. The rules decide what can be added where. — `the_rules_decide_what_can_be_added_where`
123. Undo and redo restore the text exactly. — `undo_and_redo_restore_the_text_exactly`
124. Removing and undoing keeps the same ids. — `removing_and_undoing_keeps_the_same_ids`
125. The test plan cannot be removed. — `the_test_plan_cannot_be_removed`
126. Duplicate copies children and sits after the original. — `duplicate_copies_children_and_sits_after_the_original`
127. Moving reorders and reparents and undoes. — `moving_reorders_and_reparents_and_undoes`
128. Moving between levels re indents and undoes exactly. — `moving_between_levels_re_indents_and_undoes_exactly`
129. Settings changes are undoable and a no op is not recorded. — `settings_changes_are_undoable_and_a_no_op_is_not_recorded`
130. Disabling shows in the plan. — `disabling_shows_in_the_plan`
131. Dirty follows saves and history. — `dirty_follows_saves_and_history`
132. A new edit clears redo. — `a_new_edit_clears_redo`
133. Missing elements are errors. — `missing_elements_are_errors`
*crates/plan-core/src/files.rs*

134. The recovery copy sits next to the plan. — `the_recovery_copy_sits_next_to_the_plan`
135. Save replaces the file and removes the recovery copy. — `save_replaces_the_file_and_removes_the_recovery_copy`
136. A failed save leaves the old file. — `a_failed_save_leaves_the_old_file`
137. A leftover copy is found when it differs. — `a_leftover_copy_is_found_when_it_differs`
138. A copy identical to the file is not a recovery. — `a_copy_identical_to_the_file_is_not_a_recovery`
139. A copy without its plan is still offered. — `a_copy_without_its_plan_is_still_offered`
140. An older copy is marked as not newer. — `an_older_copy_is_marked_as_not_newer`
*crates/plan-core/src/forms.rs*

141. Every curated form names settings that jmeter knows. — `every_curated_form_names_settings_that_jmeter_knows`
142. Forms exist for the everyday elements. — `forms_exist_for_the_everyday_elements`
143. Curated forms cover most of jmeters menus. — `curated_forms_cover_most_of_jmeters_menus`
144. Field properties are unique within a form except for flags in one setting. — `field_properties_are_unique_within_a_form_except_for_flags_in_one_setting`
145. Choices include the default and have unique values. — `choices_include_the_default_and_have_unique_values`
146. Every table path exists in a new element or is known. — `every_table_path_exists_in_a_new_element_or_is_known`
147. The generic form covers every simple setting. — `the_generic_form_covers_every_simple_setting`
148. Labels are made readable. — `labels_are_made_readable`
149. Java hash matches java. — `java_hash_matches_java`
150. Rows of every shape parse and escape. — `rows_of_every_shape_parse_and_escape`
151. Parameters can be added edited removed and undone. — `parameters_can_be_added_edited_removed_and_undone`
152. Response assertion patterns are a list of strings. — `response_assertion_patterns_are_a_list_of_strings`
153. Bad tables are errors. — `bad_tables_are_errors`
154. The body and the parameters table show in turn. — `the_body_and_the_parameters_table_show_in_turn`
155. The http request form has a body that is edited as text. — `the_http_request_form_has_a_body_that_is_edited_as_text`
156. The body path reaches the value of the first row. — `the_body_path_reaches_the_value_of_the_first_row`
157. A body with markup stays text. — `a_body_with_markup_stays_text`
158. Collections and groups are summaries and long text is editable. — `collections_and_groups_are_summaries_and_long_text_is_editable`
*crates/plan-core/src/fuzzy.rs*

159. Empty query keeps everything in order. — `empty_query_keeps_everything_in_order`
160. Matches subsequences and reports positions. — `matches_subsequences_and_reports_positions`
161. Better matches rank first. — `better_matches_rank_first`
162. Lower case ignores case and upper case does not. — `lower_case_ignores_case_and_upper_case_does_not`
163. Exact prefix beats scattered match. — `exact_prefix_beats_scattered_match`
164. Ties keep input order. — `ties_keep_input_order`
*crates/plan-core/src/har.rs*

165. Requests are read with their headers but not cookies or pseudo headers. — `requests_are_read_with_their_headers_but_not_cookies_or_pseudo_headers`
166. Static files and preflights are left out and counted. — `static_files_and_preflights_are_left_out_and_counted`
167. Chromes resource type decides when it is there. — `chromes_resource_type_decides_when_it_is_there`
168. Hosts can be chosen and are listed most used first. — `hosts_can_be_chosen_and_are_listed_most_used_first`
169. Bodies become a body fields or a multipart form. — `bodies_become_a_body_fields_or_a_multipart_form`
170. Pages become groups only when asked and a missing title uses the id. — `pages_become_groups_only_when_asked_and_a_missing_title_uses_the_id`
171. What is not a har file gets a sentence. — `what_is_not_a_har_file_gets_a_sentence`
172. Hostile values are cut or dropped and never break the read. — `hostile_values_are_cut_or_dropped_and_never_break_the_read`
*crates/plan-core/src/model.rs*

173. Builds the tree in document order. — `builds_the_tree_in_document_order`
174. Empty names fall back to the type label. — `empty_names_fall_back_to_the_type_label`
175. Parents children and paths. — `parents_children_and_paths`
176. Categories flags and summary. — `categories_flags_and_summary`
177. Move rules and cycles. — `move_rules_and_cycles`
178. Rejects malformed plans. — `rejects_malformed_plans`
179. Missing file error names the path. — `missing_file_error_names_the_path`
180. Props and comments are carried over. — `props_and_comments_are_carried_over`
*crates/plan-core/src/problems.rs*

181. A healthy plan has no problems. — `a_healthy_plan_has_no_problems`
182. No thread group. — `no_thread_group`
183. Thread group without requests. — `thread_group_without_requests`
184. Requests nested in controllers count. — `requests_nested_in_controllers_count`
185. Disabled thread group is a note not a warning. — `disabled_thread_group_is_a_note_not_a_warning`
186. Plugin and unknown elements are grouped by class. — `plugin_and_unknown_elements_are_grouped_by_class`
187. Disabled elements are counted. — `disabled_elements_are_counted`
188. Empty plan gives nothing. — `empty_plan_gives_nothing`
*crates/plan-core/src/rules.rs*

189. Test plan and workbench are never children. — `test_plan_and_workbench_are_never_children`
190. Thread groups and fragments only under the plan. — `thread_groups_and_fragments_only_under_the_plan`
191. Non test elements go under plan fragment or workbench. — `non_test_elements_go_under_plan_fragment_or_workbench`
192. Plan accepts config timers listeners but not samplers or controllers. — `plan_accepts_config_timers_listeners_but_not_samplers_or_controllers`
193. Controllers and thread groups accept samplers and controllers. — `controllers_and_thread_groups_accept_samplers_and_controllers`
194. Workbench accepts everything except thread groups and structure. — `workbench_accepts_everything_except_thread_groups_and_structure`
195. Samplers accept processors but not samplers or controllers. — `samplers_accept_processors_but_not_samplers_or_controllers`
196. Recorder accepts processors but not samplers. — `recorder_accepts_processors_but_not_samplers`
197. Leaf categories accept nothing. — `leaf_categories_accept_nothing`
198. Unknown parent is permissive but still blocks structural children. — `unknown_parent_is_permissive_but_still_blocks_structural_children`
199. Allowed children lists. — `allowed_children_lists`
*crates/plan-core/src/scope.rs*

200. A sampler gets its steps in jmeters order. — `a_sampler_gets_its_steps_in_jmeters_order`
201. Configuration is nearest first and the others follow their note. — `configuration_is_nearest_first_and_the_others_follow_their_note`
202. An element under a sibling sampler does not apply. — `an_element_under_a_sibling_sampler_does_not_apply`
203. Something under a sampler applies to that sampler only. — `something_under_a_sampler_applies_to_that_sampler_only`
204. Something under a controller applies to every sampler inside its parent. — `something_under_a_controller_applies_to_every_sampler_inside_its_parent`
205. A sampler applies to itself and a controller to nothing in particular. — `a_sampler_applies_to_itself_and_a_controller_to_nothing_in_particular`
206. A controller lists what it shares with everything inside. — `a_controller_lists_what_it_shares_with_everything_inside`
207. Switched off elements are listed but inactive. — `switched_off_elements_are_listed_but_inactive`
208. Nothing runs inside a test fragment. — `nothing_runs_inside_a_test_fragment`
209. Non samplers have no run order. — `non_samplers_have_no_run_order`
*crates/plan-core/src/scripts.rs*

210. A plain plan has no scripts. — `a_plain_plan_has_no_scripts`
211. Script elements are found by kind. — `script_elements_are_found_by_kind`
212. Script functions are found wherever they are written. — `script_functions_are_found_wherever_they_are_written`
213. The question names how many places and what kinds. — `the_question_names_how_many_places_and_what_kinds`
214. Every corpus plan is scanned without trouble. — `every_corpus_plan_is_scanned_without_trouble`
*crates/plan-core/src/search.rs*

215. Finds by name. — `finds_by_name`
216. Finds by type when the name does not match. — `finds_by_type_when_the_name_does_not_match`
217. Name matches rank above type matches. — `name_matches_rank_above_type_matches`
218. Empty query and limit. — `empty_query_and_limit`
*crates/plan-core/src/starters.rs*

219. Every starter is a plan that opens clean and saves back unchanged. — `every_starter_is_a_plan_that_opens_clean_and_saves_back_unchanged`
220. The empty plan is only a test plan. — `the_empty_plan_is_only_a_test_plan`
221. The api starter has what the description says. — `the_api_starter_has_what_the_description_says`
222. The web starter has what the description says. — `the_web_starter_has_what_the_description_says`
223. Starters can be edited further like any plan. — `starters_can_be_edited_further_like_any_plan`
224. The starters start without warnings or undefined variables. — `the_starters_start_without_warnings_or_undefined_variables`
225. Names with markup are written as text not as xml. — `names_with_markup_are_written_as_text_not_as_xml`
226. Control characters are dropped and blank names get a default. — `control_characters_are_dropped_and_blank_names_get_a_default`
227. Building a starter is quick. — `building_a_starter_is_quick`
228. Labels and file names are distinct and sensible. — `labels_and_file_names_are_distinct_and_sensible`
*crates/plan-core/src/templates.rs*

229. Every menu entry loads. — `every_menu_entry_loads`
230. Entries are unique by tag and gui. — `entries_are_unique_by_tag_and_gui`
231. Menu groups match the catalog. — `menu_groups_match_the_catalog`
232. Http request has what the gui gives it. — `http_request_has_what_the_gui_gives_it`
233. Listeners with the same tag are told apart by gui. — `listeners_with_the_same_tag_are_told_apart_by_gui`
*crates/plan-core/src/tree.rs*

234. Collapsed tree shows only roots. — `collapsed_tree_shows_only_roots`
235. Expanded to depth. — `expanded_to_depth`
236. Toggle and row flags. — `toggle_and_row_flags`
237. Select expands ancestors. — `select_expands_ancestors`
238. Collapsing an ancestor moves the selection to it. — `collapsing_an_ancestor_moves_the_selection_to_it`
239. Expand all and collapse all. — `expand_all_and_collapse_all`
240. Arrow navigation stops at the ends. — `arrow_navigation_stops_at_the_ends`
241. Up with no selection picks the last row and pages clamp. — `up_with_no_selection_picks_the_last_row_and_pages_clamp`
242. Left and right keys. — `left_and_right_keys`
243. Selecting a missing node is ignored. — `selecting_a_missing_node_is_ignored`
*crates/plan-core/src/variables.rs*

244. Names come from settings tables and the test plan. — `names_come_from_settings_tables_and_the_test_plan`
245. Scripts can define variables. — `scripts_can_define_variables`
246. Only undefined names are reported once per element. — `only_undefined_names_are_reported_once_per_element`
247. What an extractor adds to a name counts as defined. — `what_an_extractor_adds_to_a_name_counts_as_defined`
248. Functions and odd text are left alone. — `functions_and_odd_text_are_left_alone`
249. Built in names are known. — `built_in_names_are_known`
250. A plan in which everything is defined has no hints. — `a_plan_in_which_everything_is_defined_has_no_hints`
*crates/plan-core/tests/copy_paste.rs*

251. A copy is a plan file that holds just the elements. — `a_copy_is_a_plan_file_that_holds_just_the_elements`
252. The test plan cannot be copied and nothing cannot either. — `the_test_plan_cannot_be_copied_and_nothing_cannot_either`
253. Copying changes nothing. — `copying_changes_nothing`
254. Several elements copy in the order given. — `several_elements_copy_in_the_order_given`
255. A pasted subtree is the same as the copied one and the original is untouched. — `a_pasted_subtree_is_the_same_as_the_copied_one_and_the_original_is_untouched`
256. Pasting gives every new element its own id. — `pasting_gives_every_new_element_its_own_id`
257. The same text can be pasted again and again. — `the_same_text_can_be_pasted_again_and_again`
258. The position decides where it goes. — `the_position_decides_where_it_goes`
259. A paste is one undo step and redo brings it back. — `a_paste_is_one_undo_step_and_redo_brings_it_back`
260. A whole plan pasted brings what is inside its test plan. — `a_whole_plan_pasted_brings_what_is_inside_its_test_plan`
261. Paste near goes under a container and next to a leaf. — `paste_near_goes_under_a_container_and_next_to_a_leaf`
262. Paste near looks further up when it does not fit next to the selection. — `paste_near_looks_further_up_when_it_does_not_fit_next_to_the_selection`
263. What cannot go there is refused and changes nothing. — `what_cannot_go_there_is_refused_and_changes_nothing`
264. A mixed paste is all or nothing. — `a_mixed_paste_is_all_or_nothing`
265. Text that is not a plan is refused with a plain message. — `text_that_is_not_a_plan_is_refused_with_a_plain_message`
266. A test plan with nothing in it is refused. — `a_test_plan_with_nothing_in_it_is_refused`
267. Hostile texts are refused or read safely. — `hostile_texts_are_refused_or_read_safely`
268. An element with markup in its name stays one element. — `an_element_with_markup_in_its_name_stays_one_element`
269. A paste with too many elements or too much text is refused. — `a_paste_with_too_many_elements_or_too_much_text_is_refused`
270. Elements of plugins are copied and pasted like any other. — `elements_of_plugins_are_copied_and_pasted_like_any_other`
271. Copying and pasting every element of every sample plan works and undoes exactly. — `copying_and_pasting_every_element_of_every_sample_plan_works_and_undoes_exactly`
272. A copy of a whole thread group pasted into another plan runs the same shape. — `a_copy_of_a_whole_thread_group_pasted_into_another_plan_runs_the_same_shape`
273. Copying and pasting a big subtree is quick. — `copying_and_pasting_a_big_subtree_is_quick`
274. Damaged copies are refused or pasted cleanly and never change a plan by halves. — `damaged_copies_are_refused_or_pasted_cleanly_and_never_change_a_plan_by_halves`
*crates/plan-core/tests/corpus.rs*

275. Simple jmeter sample. — `simple_jmeter_sample`
276. Web test template has the usual structure. — `web_test_template_has_the_usual_structure`
277. Plugin plan keeps plugin elements. — `plugin_plan_keeps_plugin_elements`
278. Every real parent child pair obeys the rules. — `every_real_parent_child_pair_obeys_the_rules`
279. Every core tag in the catalog has a label and icon. — `every_core_tag_in_the_catalog_has_a_label_and_icon`
280. A very large plan loads and expands quickly. — `a_very_large_plan_loads_and_expands_quickly`
*crates/plan-core/tests/derive_run.rs*

281. Validate runs every sampler once with one user and no waiting. — `validate_runs_every_sampler_once_with_one_user_and_no_waiting`
282. Send once sends exactly one request. — `send_once_sends_exactly_one_request`
283. Send once of each request in a plan works. — `send_once_of_each_request_in_a_plan_works`
*crates/plan-core/tests/edit_corpus.rs*

284. Random edits on real plans stay valid and undo exactly. — `random_edits_on_real_plans_stay_valid_and_undo_exactly`
285. Editing a large plan is fast enough. — `editing_a_large_plan_is_fast_enough`
*crates/plan-core/tests/har_plan.rs*

286. Pages become transaction controllers holding their requests. — `pages_become_transaction_controllers_holding_their_requests`
287. Without grouping the requests sit directly under the group. — `without_grouping_the_requests_sit_directly_under_the_group`
288. A page that comes back later starts a new controller and requests without a page stay loose. — `a_page_that_comes_back_later_starts_a_new_controller_and_requests_without_a_page_stay_loose`
289. A big recording pastes quickly. — `a_big_recording_pastes_quickly`
290. Hostile har files never make text that cannot be pasted. — `hostile_har_files_never_make_text_that_cannot_be_pasted`
*crates/plan-core/tests/inherited.rs*

291. The nearest defaults with a value supply it and are named. — `the_nearest_defaults_with_a_value_supply_it_and_are_named`
292. Defaults that are switched off or empty supply nothing. — `defaults_that_are_switched_off_or_empty_supply_nothing`
293. Only the settings defaults can supply are looked up. — `only_the_settings_defaults_can_supply_are_looked_up`
294. Something that is not a request or has no defaults gets nothing. — `something_that_is_not_a_request_or_has_no_defaults_gets_nothing`
295. The real plan from the jmeter samples works. — `the_real_plan_from_the_jmeter_samples_works`
*crates/plan-core/tests/jmeter_load.rs*

296. A plan with every element loads in jmeter. — `a_plan_with_every_element_loads_in_jmeter`
*crates/plan-core/tests/mutations.rs*

297. Damaged plans load or are refused and a loaded plan can always be walked. — `damaged_plans_load_or_are_refused_and_a_loaded_plan_can_always_be_walked`
*crates/plan-core/tests/security.rs*

298. Hostile text through every edit keeps the plan structure. — `hostile_text_through_every_edit_keeps_the_plan_structure`
299. A plan nested as deep as allowed still builds and edits. — `a_plan_nested_as_deep_as_allowed_still_builds_and_edits`
300. A plan nested past the limit is refused with a message. — `a_plan_nested_past_the_limit_is_refused_with_a_message`
301. A file over the size limit is refused without reading it. — `a_file_over_the_size_limit_is_refused_without_reading_it`
302. A file that is not text is an error not a crash. — `a_file_that_is_not_text_is_an_error_not_a_crash`
303. Saving never writes through a leftover temporary file to another file. — `saving_never_writes_through_a_leftover_temporary_file_to_another_file`
304. Values that span several lines survive duplicate move and add. — `values_that_span_several_lines_survive_duplicate_move_and_add`
305. Saving a plan that is a link writes through the link. — `saving_a_plan_that_is_a_link_writes_through_the_link`
306. The recovery copy is written next to this plan only. — `the_recovery_copy_is_written_next_to_this_plan_only`
307. Saving to a read only file fails cleanly and leaves no temporary file. — `saving_to_a_read_only_file_fails_cleanly_and_leaves_no_temporary_file`
308. Saving into a missing folder is an error. — `saving_into_a_missing_folder_is_an_error`
309. A very long path gives an error or works but never panics. — `a_very_long_path_gives_an_error_or_works_but_never_panics`
310. A reserved windows device name is never reported as saved. — `a_reserved_windows_device_name_is_never_reported_as_saved`
311. A recovery copy that is not text is ignored. — `a_recovery_copy_that_is_not_text_is_ignored`
*crates/plan-core/tests/starters_new.rs*

312. The login template keeps the token and sends it. — `the_login_template_keeps_the_token_and_sends_it`
313. The data template uses a term from each row. — `the_data_template_uses_a_term_from_each_row`
314. The load template runs for ten minutes after a minute of ramp up. — `the_load_template_runs_for_ten_minutes_after_a_minute_of_ramp_up`
315. The spike template adds a burst after a delay. — `the_spike_template_adds_a_burst_after_a_delay`
316. No template starts with a warning and each has a distinct button and file name. — `no_template_starts_with_a_warning_and_each_has_a_distinct_button_and_file_name`
317. A name with markup stays text in every template. — `a_name_with_markup_stays_text_in_every_template`
318. Every template is a plan the real jmeter runs without a failed request. — `every_template_is_a_plan_the_real_jmeter_runs_without_a_failed_request`
*crates/plan-core/tests/think_times.rs*

319. Every enabled request without a pause gets one and the others are left alone. — `every_enabled_request_without_a_pause_gets_one_and_the_others_are_left_alone`
320. It is one step to undo and redo. — `it_is_one_step_to_undo_and_redo`
321. Only what is under the chosen element is touched. — `only_what_is_under_the_chosen_element_is_touched`
322. The same range makes a constant timer. — `the_same_range_makes_a_constant_timer`
323. Asking again or with nothing to do or a wrong range says why and changes nothing. — `asking_again_or_with_nothing_to_do_or_a_wrong_range_says_why_and_changes_nothing`
324. A big plan gets its pauses quickly. — `a_big_plan_gets_its_pauses_quickly`

### shell-core (settings, layout, text editing, palette, actions)

*crates/shell-core/src/actions.rs*

325. All lists every action once. — `all_lists_every_action_once`
326. Shortcuts are unique. — `shortcuts_are_unique`
327. Nothing open. — `nothing_open`
328. With a plan and tabs. — `with_a_plan_and_tabs`
329. Next tab needs two tabs. — `next_tab_needs_two_tabs`
330. Run and stop follow the run state. — `run_and_stop_follow_the_run_state`
331. Validate and send once need a plan a runtime and no run going. — `validate_and_send_once_need_a_plan_a_runtime_and_no_run_going`
332. Stop now follows the run state. — `stop_now_follows_the_run_state`
333. Only a chosen runs folder can be cleared and not during a run. — `only_a_chosen_runs_folder_can_be_cleared_and_not_during_a_run`
334. Think times need a selection that holds requests and an editable plan. — `think_times_need_a_selection_that_holds_requests_and_an_editable_plan`
335. A har file needs an open plan that can be edited. — `a_har_file_needs_an_open_plan_that_can_be_edited`
336. Run properties belong to an open plan. — `run_properties_belong_to_an_open_plan`
337. Results can be opened any time and a report needs a finished run. — `results_can_be_opened_any_time_and_a_report_needs_a_finished_run`
338. An expression can be tried when java and jmeter are there. — `an_expression_can_be_tried_when_java_and_jmeter_are_there`
339. Requests can be viewed once there is a run. — `requests_can_be_viewed_once_there_is_a_run`
340. Saving needs unsaved changes and save as only a plan. — `saving_needs_unsaved_changes_and_save_as_only_a_plan`
341. Undo and redo follow the history. — `undo_and_redo_follow_the_history`
342. Element commands need a selection the test plan is special. — `element_commands_need_a_selection_the_test_plan_is_special`
343. Every group has actions. — `every_group_has_actions`
344. Adding follows the edit capability. — `adding_follows_the_edit_capability`
*crates/shell-core/src/highlight.rs*

345. The language is guessed from the first character. — `the_language_is_guessed_from_the_first_character`
346. Json keys values numbers and keywords. — `json_keys_values_numbers_and_keywords`
347. Json strings with escapes and a missing end. — `json_strings_with_escapes_and_a_missing_end`
348. Json words that are not keywords stay plain. — `json_words_that_are_not_keywords_stay_plain`
349. Xml tags attributes and text. — `xml_tags_attributes_and_text`
350. Xml comments run to their end or the end of the line. — `xml_comments_run_to_their_end_or_the_end_of_the_line`
351. Variables are marked in any language. — `variables_are_marked_in_any_language`
352. Every line is covered without gaps whatever it holds. — `every_line_is_covered_without_gaps_whatever_it_holds`
353. Random lines never break the rules. — `random_lines_never_break_the_rules`
354. A very long line is not coloured and is quick. — `a_very_long_line_is_not_coloured_and_is_quick`
355. Many variables in a line are quick. — `many_variables_in_a_line_are_quick`
*crates/shell-core/src/layout.rs*

356. Starts empty with panels from settings. — `starts_empty_with_panels_from_settings`
357. The inspector is hidden by default. — `the_inspector_is_hidden_by_default`
358. Opening a tab places it after the active one. — `opening_a_tab_places_it_after_the_active_one`
359. A plan never gets a second tab. — `a_plan_never_gets_a_second_tab`
360. Closing activates the right neighbour then the left. — `closing_activates_the_right_neighbour_then_the_left`
361. Closing a tab before the active one keeps it active. — `closing_a_tab_before_the_active_one_keeps_it_active`
362. Next and previous wrap. — `next_and_previous_wrap`
363. Close others and close all. — `close_others_and_close_all`
364. Rename and dirty apply to the matching tab only. — `rename_and_dirty_apply_to_the_matching_tab_only`
365. Panel sizes clamp and ignore nan. — `panel_sizes_clamp_and_ignore_nan`
366. Toggles and saving back to settings. — `toggles_and_saving_back_to_settings`
*crates/shell-core/src/lines.rs*

367. An empty text is one empty line. — `an_empty_text_is_one_empty_line`
368. Lines are found by number and by offset. — `lines_are_found_by_number_and_by_offset`
369. A final line break starts an empty line. — `a_final_line_break_starts_an_empty_line`
370. Lines past the end are the last line. — `lines_past_the_end_are_the_last_line`
371. Digits follow the line count. — `digits_follow_the_line_count`
372. Many lines are indexed quickly. — `many_lines_are_indexed_quickly`
*crates/shell-core/src/number.rs*

373. Parses numbers. — `parses_numbers`
374. Rejects bad input with a reason. — `rejects_bad_input_with_a_reason`
375. Variables and functions are valid. — `variables_and_functions_are_valid`
376. Empty is allowed only when optional. — `empty_is_allowed_only_when_optional`
377. Partial input is accepted while typing. — `partial_input_is_accepted_while_typing`
378. Partial expressions are accepted. — `partial_expressions_are_accepted`
379. Stepping. — `stepping`
380. Expressions and junk are not stepped. — `expressions_and_junk_are_not_stepped`
381. Formatting. — `formatting`
382. Clamp handles open ranges. — `clamp_handles_open_ranges`
*crates/shell-core/src/palette.rs*

383. Prefixes pick the mode. — `prefixes_pick_the_mode`
384. Commands mode lists only actions. — `commands_mode_lists_only_actions`
385. Unavailable commands sink to the bottom with a reason. — `unavailable_commands_sink_to_the_bottom_with_a_reason`
386. Elements mode finds by name and carries the type. — `elements_mode_finds_by_name_and_carries_the_type`
387. Elements mode without a query lists the tree in order. — `elements_mode_without_a_query_lists_the_tree_in_order`
388. Plain queries mix commands recents and elements. — `plain_queries_mix_commands_recents_and_elements`
389. An empty query shows commands and a few recents but no elements. — `an_empty_query_shows_commands_and_a_few_recents_but_no_elements`
390. Recent items use the file name as the title. — `recent_items_use_the_file_name_as_the_title`
391. Add mode offers only what fits under the selection. — `add_mode_offers_only_what_fits_under_the_selection`
392. Add mode with a query also lists what does not fit as unavailable. — `add_mode_with_a_query_also_lists_what_does_not_fit_as_unavailable`
393. Add mode quick list comes first without a query. — `add_mode_quick_list_comes_first_without_a_query`
394. Add mode without a selection uses the test plan. — `add_mode_without_a_selection_uses_the_test_plan`
395. Add mode under a leaf offers nothing. — `add_mode_under_a_leaf_offers_nothing`
396. Add items are unavailable until editing exists. — `add_items_are_unavailable_until_editing_exists`
397. Limit is applied after ranking. — `limit_is_applied_after_ranking`
398. Shortcuts are included for commands. — `shortcuts_are_included_for_commands`
*crates/shell-core/src/settings.rs*

399. Properties are kept for a plan file and an empty text removes them. — `properties_are_kept_for_a_plan_file_and_an_empty_text_removes_them`
400. Kept properties are limited and a damaged file cannot break them. — `kept_properties_are_limited_and_a_damaged_file_cannot_break_them`
401. Missing file gives defaults without a notice. — `missing_file_gives_defaults_without_a_notice`
402. Round trip. — `round_trip`
403. Older files with missing fields still load. — `older_files_with_missing_fields_still_load`
404. Unknown fields are ignored. — `unknown_fields_are_ignored`
405. Damaged file is set aside. — `damaged_file_is_set_aside`
406. Wrong types count as damage. — `wrong_types_count_as_damage`
407. Sizes are clamped on load and save. — `sizes_are_clamped_on_load_and_save`
408. Recent files are unique ordered and capped. — `recent_files_are_unique_ordered_and_capped`
409. A settings file saved with a byte order mark is read. — `a_settings_file_saved_with_a_byte_order_mark_is_read`
410. Run history defaults to the plan folder and ten runs. — `run_history_defaults_to_the_plan_folder_and_ten_runs`
411. Older files without run history settings still load. — `older_files_without_run_history_settings_still_load`
412. Run history settings round trip and are clamped. — `run_history_settings_round_trip_and_are_clamped`
413. Script approval is remembered per plan file and ignores case and slashes. — `script_approval_is_remembered_per_plan_file_and_ignores_case_and_slashes`
414. Default path ends in the app folder. — `default_path_ends_in_the_app_folder`
*crates/shell-core/src/text_buffer.rs*

415. Typing appends at the caret. — `typing_appends_at_the_caret`
416. Inserting nothing changes nothing. — `inserting_nothing_changes_nothing`
417. Typing replaces the selection. — `typing_replaces_the_selection`
418. Backspace and delete. — `backspace_and_delete`
419. Selection is deleted as a whole. — `selection_is_deleted_as_a_whole`
420. Arrows collapse a selection to its edge. — `arrows_collapse_a_selection_to_its_edge`
421. Shift arrows extend in either direction. — `shift_arrows_extend_in_either_direction`
422. Movement stops at the ends. — `movement_stops_at_the_ends`
423. Words. — `words`
424. Delete by word. — `delete_by_word`
425. Double click selects a word. — `double_click_selects_a_word`
426. Graphemes are never split. — `graphemes_are_never_split`
427. Positions past the end are clamped. — `positions_past_the_end_are_clamped`
428. Newlines become spaces. — `newlines_become_spaces`
429. Filter and length limit. — `filter_and_length_limit`
430. Typing in a row is one undo step. — `typing_in_a_row_is_one_undo_step`
431. Moving the caret ends the typing run. — `moving_the_caret_ends_the_typing_run`
432. Paste cut and delete are separate steps. — `paste_cut_and_delete_are_separate_steps`
433. New edits clear redo. — `new_edits_clear_redo`
434. Backspaces in a row coalesce. — `backspaces_in_a_row_coalesce`
435. Copy needs a selection. — `copy_needs_a_selection`
436. Replace range for input methods. — `replace_range_for_input_methods`
437. Set text resets history. — `set_text_resets_history`
438. Undo history is bounded. — `undo_history_is_bounded`
439. A multiline buffer keeps its lines and unifies line breaks. — `a_multiline_buffer_keeps_its_lines_and_unifies_line_breaks`
440. Characters a plan file cannot hold are dropped. — `characters_a_plan_file_cannot_hold_are_dropped`
441. Lines are counted and found. — `lines_are_counted_and_found`
442. Columns count whole characters. — `columns_count_whole_characters`
443. Up and down keep the column across short lines. — `up_and_down_keep_the_column_across_short_lines`
444. Up from the first line goes to the start and down from the last to the end. — `up_from_the_first_line_goes_to_the_start_and_down_from_the_last_to_the_end`
445. Shift with up and down selects lines. — `shift_with_up_and_down_selects_lines`
446. Moving many lines at once for page up and down. — `moving_many_lines_at_once_for_page_up_and_down`
447. Up and down do nothing in a one line buffer. — `up_and_down_do_nothing_in_a_one_line_buffer`
448. Home and end work per line in a multiline buffer. — `home_and_end_work_per_line_in_a_multiline_buffer`
449. A triple click selects the whole line. — `a_triple_click_selects_the_whole_line`
450. Enter copies the indent and goes one deeper after an opening bracket. — `enter_copies_the_indent_and_goes_one_deeper_after_an_opening_bracket`
451. Enter replaces a selection and works in the middle of a line. — `enter_replaces_a_selection_and_works_in_the_middle_of_a_line`
452. Tab moves to the next stop and shift tab takes an indent off. — `tab_moves_to_the_next_stop_and_shift_tab_takes_an_indent_off`
453. Tab and shift tab shift every line of a selection. — `tab_and_shift_tab_shift_every_line_of_a_selection`
454. A selection that ends after a line break does not shift the next line. — `a_selection_that_ends_after_a_line_break_does_not_shift_the_next_line`
455. Indenting a one line buffer does nothing. — `indenting_a_one_line_buffer_does_nothing`
456. The version changes with the text and not with the caret. — `the_version_changes_with_the_text_and_not_with_the_caret`
457. A big text keeps fewer undo steps but always the latest. — `a_big_text_keeps_fewer_undo_steps_but_always_the_latest`
458. Moving through a big text is quick. — `moving_through_a_big_text_is_quick`
459. Typing into a big text is quick. — `typing_into_a_big_text_is_quick`
460. A huge paste is kept whole and cleaned in one pass. — `a_huge_paste_is_kept_whole_and_cleaned_in_one_pass`
461. Random operations keep the buffer sound and undo gives everything back. — `random_operations_keep_the_buffer_sound_and_undo_gives_everything_back`
*crates/shell-core/tests/settings_run.rs*

462. Defaults match jmeters own. — `defaults_match_jmeters_own`
463. Values round trip. — `values_round_trip`
464. Startup is stored as a word. — `startup_is_stored_as_a_word`
465. Out of range values are clamped. — `out_of_range_values_are_clamped`
466. An unknown startup word counts as damage. — `an_unknown_startup_word_counts_as_damage`
467. Old files without the new fields get defaults. — `old_files_without_the_new_fields_get_defaults`

### jvm-runtime (finding Java and JMeter, running and stopping JMeter, results, live numbers, run folders)

*crates/jvm-runtime/src/details.rs*

468. The variables of a request are read and limited. — `the_variables_of_a_request_are_read_and_limited`
469. A line from the listener is read whole. — `a_line_from_the_listener_is_read_whole`
470. The request and the response are laid out to read. — `the_request_and_the_response_are_laid_out_to_read`
471. A get has no body and a binary response is not shown. — `a_get_has_no_body_and_a_binary_response_is_not_shown`
472. A body that looks like the markers is cut at the first real one. — `a_body_that_looks_like_the_markers_is_cut_at_the_first_real_one`
473. Lines are read as they arrive and a half line waits. — `lines_are_read_as_they_arrive_and_a_half_line_waits`
474. Damaged lines are skipped and counted and the rest is read. — `damaged_lines_are_skipped_and_counted_and_the_rest_is_read`
475. Missing fields are empty and unknown ones are ignored. — `missing_fields_are_empty_and_unknown_ones_are_ignored`
476. Texts in a line are limited whatever the file says. — `texts_in_a_line_are_limited_whatever_the_file_says`
477. A line that is too long or never ends is dropped without using the memory. — `a_line_that_is_too_long_or_never_ends_is_dropped_without_using_the_memory`
478. No more than the limit is kept in memory. — `no_more_than_the_limit_is_kept_in_memory`
479. A shorter file starts the reading again. — `a_shorter_file_starts_the_reading_again`
480. The properties name the mode the file and a limit that cannot be raised. — `the_properties_name_the_mode_the_file_and_a_limit_that_cannot_be_raised`
481. The script keeps to what it promises. — `the_script_keeps_to_what_it_promises`
*crates/jvm-runtime/src/live.rs*

482. The users of a run are the most seen and not the last. — `the_users_of_a_run_are_the_most_seen_and_not_the_last`
483. Totals are exact. — `totals_are_exact`
484. Percentiles use the nearest rank. — `percentiles_use_the_nearest_rank`
485. One sample has every percentile. — `one_sample_has_every_percentile`
486. Throughput is samples over the time covered. — `throughput_is_samples_over_the_time_covered`
487. Labels are kept apart and capped. — `labels_are_kept_apart_and_capped`
488. The series groups samples by the second they finished. — `the_series_groups_samples_by_the_second_they_finished`
489. A late row goes into the step it belongs to. — `a_late_row_goes_into_the_step_it_belongs_to`
490. A long run merges steps instead of growing. — `a_long_run_merges_steps_instead_of_growing`
491. Only the latest errors are kept. — `only_the_latest_errors_are_kept`
492. Absurd values do not break anything. — `absurd_values_do_not_break_anything`
493. A finished results file is read whole. — `a_finished_results_file_is_read_whole`
494. Empty stats are all zero. — `empty_stats_are_all_zero`
495. A million samples are added fast and in bounded memory. — `a_million_samples_are_added_fast_and_in_bounded_memory`
*crates/jvm-runtime/src/locate.rs*

496. Java versions. — `java_versions`
497. A script is never used as java. — `a_script_is_never_used_as_java`
498. A bare program name is found on the path and other paths are kept. — `a_bare_program_name_is_found_on_the_path_and_other_paths_are_kept`
*crates/jvm-runtime/src/props.rs*

499. Lines of name and value are read and the rest is skipped. — `lines_of_name_and_value_are_read_and_the_rest_is_skipped`
500. Every kind of wrong line says which line and why. — `every_kind_of_wrong_line_says_which_line_and_why`
501. Values are limited and cannot hold control characters. — `values_are_limited_and_cannot_hold_control_characters`
502. The note names the properties and never shows values. — `the_note_names_the_properties_and_never_shows_values`
*crates/jvm-runtime/src/report.rs*

503. The report goes in a folder next to the results named after them. — `the_report_goes_in_a_folder_next_to_the_results_named_after_them`
504. What cannot be done is refused before anything starts. — `what_cannot_be_done_is_refused_before_anything_starts`
505. Times are said in words. — `times_are_said_in_words`
506. The reason is taken from what jmeter printed. — `the_reason_is_taken_from_what_jmeter_printed`
*crates/jvm-runtime/src/results.rs*

507. Summary interval line. — `summary_interval_line`
508. Summary cumulative line has no threads. — `summary_cumulative_line_has_no_threads`
509. Summary rate when elapsed is zero. — `summary_rate_when_elapsed_is_zero`
510. Stop port line. — `stop_port_line`
511. Jtl tail handles partial lines and quotes. — `jtl_tail_handles_partial_lines_and_quotes`
*crates/jvm-runtime/src/run.rs*

512. A process ended by a signal still has an exit code. — `a_process_ended_by_a_signal_still_has_an_exit_code`
513. The long windows form of a path is shortened. — `the_long_windows_form_of_a_path_is_shortened`
*crates/jvm-runtime/src/runs.rs*

514. Runs go in a runs folder next to the plan or inside the chosen folder. — `runs_go_in_a_runs_folder_next_to_the_plan_or_inside_the_chosen_folder`
515. A custom place needs a folder otherwise the plan folder is used. — `a_custom_place_needs_a_folder_otherwise_the_plan_folder_is_used`
516. Runs of a plan are found however the plan path is spelled. — `runs_of_a_plan_are_found_however_the_plan_path_is_spelled`
517. Preparing makes the runs folder and checks it can be written. — `preparing_makes_the_runs_folder_and_checks_it_can_be_written`
518. A missing or wrong custom folder is reported. — `a_missing_or_wrong_custom_folder_is_reported`
519. A new run gets its own folder named after the plan and time. — `a_new_run_gets_its_own_folder_named_after_the_plan_and_time`
520. Plan names that try to leave the folder stay inside it. — `plan_names_that_try_to_leave_the_folder_stay_inside_it`
521. Listing shows this apps run folders newest first and ignores the rest. — `listing_shows_this_apps_run_folders_newest_first_and_ignores_the_rest`
522. A damaged or huge run file is ignored. — `a_damaged_or_huge_run_file_is_ignored`
523. Only the newest runs of a plan are kept with all their files. — `only_the_newest_runs_of_a_plan_are_kept_with_all_their_files`
524. A run that has not ended is never pruned and keep is at least one. — `a_run_that_has_not_ended_is_never_pruned_and_keep_is_at_least_one`
525. Clearing removes only this apps run folders. — `clearing_removes_only_this_apps_run_folders`
526. Clearing refuses a folder that is not named runs. — `clearing_refuses_a_folder_that_is_not_named_runs`
527. Clearing never follows a link out of the folder. — `clearing_never_follows_a_link_out_of_the_folder`
528. Folder size adds up the files. — `folder_size_adds_up_the_files`
*crates/jvm-runtime/src/tester.rs*

529. What cannot be tried is refused before anything starts. — `what_cannot_be_tried_is_refused_before_anything_starts`
530. Every kind has a word and a label. — `every_kind_has_a_word_and_a_label`
531. The program reads only its input and fetches nothing. — `the_program_reads_only_its_input_and_fetches_nothing`
*crates/jvm-runtime/tests/details_jmeter.rs*

532. Failed requests keep what was sent and what came back. — `failed_requests_keep_what_was_sent_and_what_came_back`
533. All mode keeps the successful ones too up to the limit. — `all_mode_keeps_the_successful_ones_too_up_to_the_limit`
534. Off keeps nothing and without the listener nothing is written. — `off_keeps_nothing_and_without_the_listener_nothing_is_written`
535. A posted body and the headers sent are kept. — `a_posted_body_and_the_headers_sent_are_kept`
536. Texts with quotes lines and odd characters make valid lines that read back exactly. — `texts_with_quotes_lines_and_odd_characters_make_valid_lines_that_read_back_exactly`
537. A long response is cut and a response that is not text is not kept. — `a_long_response_is_cut_and_a_response_that_is_not_text_is_not_kept`
538. The limit holds however many threads fail at once. — `the_limit_holds_however_many_threads_fail_at_once`
539. All the failures under the limit are kept when many threads fail at once. — `all_the_failures_under_the_limit_are_kept_when_many_threads_fail_at_once`
540. A listener that cannot write does not change the run. — `a_listener_that_cannot_write_does_not_change_the_run`
541. The listener does not change what the requests do. — `the_listener_does_not_change_what_the_requests_do`
542. A run with the listener is about as fast as one without. — `a_run_with_the_listener_is_about_as_fast_as_one_without`
543. The variables of a request are kept without jmeters own. — `the_variables_of_a_request_are_kept_without_jmeters_own`
*crates/jvm-runtime/tests/mutations.rs*

544. Damaged results files never crash the reader. — `damaged_results_files_never_crash_the_reader`
545. A file written in pieces reads the same as the whole file. — `a_file_written_in_pieces_reads_the_same_as_the_whole_file`
546. Damaged console lines never crash the parsers. — `damaged_console_lines_never_crash_the_parsers`
*crates/jvm-runtime/tests/report_jmeter.rs*

547. A results file becomes a report that opens in a browser. — `a_results_file_becomes_a_report_that_opens_in_a_browser`
548. The results of a real run of this app make a report. — `the_results_of_a_real_run_of_this_app_make_a_report`
549. Folders and files with odd names work. — `folders_and_files_with_odd_names_work`
550. A folder that already has files is refused and left alone. — `a_folder_that_already_has_files_is_refused_and_left_alone`
551. A file that is not a results file gives a sentence and leaves no half report. — `a_file_that_is_not_a_results_file_gives_a_sentence_and_leaves_no_half_report`
552. A report that takes too long is stopped and removed. — `a_report_that_takes_too_long_is_stopped_and_removed`
553. A large results file makes a report in reasonable time. — `a_large_results_file_makes_a_report_in_reasonable_time`
*crates/jvm-runtime/tests/results_security.rs*

554. Rows are read with every column. — `rows_are_read_with_every_column`
555. Crlf line ends and quoted fields with commas quotes and line breaks are read. — `crlf_line_ends_and_quoted_fields_with_commas_quotes_and_line_breaks_are_read`
556. Rows written in pieces are found when complete. — `rows_written_in_pieces_are_found_when_complete`
557. A missing file gives no rows and no error. — `a_missing_file_gives_no_rows_and_no_error`
558. Missing columns and absurd numbers become zero not errors. — `missing_columns_and_absurd_numbers_become_zero_not_errors`
559. A record larger than the limit is dropped and the rows after it are kept. — `a_record_larger_than_the_limit_is_dropped_and_the_rows_after_it_are_kept`
560. A line that never ends does not grow memory. — `a_line_that_never_ends_does_not_grow_memory`
561. An open quote that never closes is dropped too. — `an_open_quote_that_never_closes_is_dropped_too`
562. Text that is not a results file gives no samples. — `text_that_is_not_a_results_file_gives_no_samples`
563. Binary junk is survived. — `binary_junk_is_survived`
564. A huge file is read in bounded steps. — `a_huge_file_is_read_in_bounded_steps`
565. Text with accents and emoji survives the read limit boundary. — `text_with_accents_and_emoji_survives_the_read_limit_boundary`
566. A file replaced by a shorter one is read from the start. — `a_file_replaced_by_a_shorter_one_is_read_from_the_start`
567. Half a million rows are read quickly. — `half_a_million_rows_are_read_quickly`
*crates/jvm-runtime/tests/run_jmeter.rs*

568. A normal run is accepted. — `a_normal_run_is_accepted`
569. A script is refused as the java program. — `a_script_is_refused_as_the_java_program`
570. Property names that could add options or commands are refused. — `property_names_that_could_add_options_or_commands_are_refused`
571. Property values and paths with control characters are refused. — `property_values_and_paths_with_control_characters_are_refused`
572. Only memory and define options are passed to java. — `only_memory_and_define_options_are_passed_to_java`
573. Missing pieces are named in the message. — `missing_pieces_are_named_in_the_message`
574. Starting with a refused spec starts nothing. — `starting_with_a_refused_spec_starts_nothing`
575. A real run reports every sample and ends cleanly. — `a_real_run_reports_every_sample_and_ends_cleanly`
576. Folders and plans with spaces quotes and symbols still run. — `folders_and_plans_with_spaces_quotes_and_symbols_still_run`
577. Paths with letters outside the system code page get a warning. — `paths_with_letters_outside_the_system_code_page_get_a_warning`
578. A plan whose name looks like an option is still a plan. — `a_plan_whose_name_looks_like_an_option_is_still_a_plan`
579. Stop lets a run that never ends finish by itself. — `stop_lets_a_run_that_never_ends_finish_by_itself`
580. Stop now ends a run quickly. — `stop_now_ends_a_run_quickly`
581. Kill ends a run that is not listening. — `kill_ends_a_run_that_is_not_listening`
582. Dropping the handle never leaves jmeter running. — `dropping_the_handle_never_leaves_jmeter_running`
583. The stop port answers this machine only. — `the_stop_port_answers_this_machine_only`
584. Each run gets its own stop port. — `each_run_gets_its_own_stop_port`
585. A flood of output is bounded and does not block the run. — `a_flood_of_output_is_bounded_and_does_not_block_the_run`
586. A run with a bad plan reports the failure. — `a_run_with_a_bad_plan_reports_the_failure`
587. Time from start to the first sample is measured. — `time_from_start_to_the_first_sample_is_measured`
588. Jmeter is ended when the app crashes. — `jmeter_is_ended_when_the_app_crashes`
*crates/jvm-runtime/tests/tester_jmeter.rs*

589. Json path finds values lists and nothing. — `json_path_finds_values_lists_and_nothing`
590. Json path says what is wrong. — `json_path_says_what_is_wrong`
591. A regular expression shows each match with its groups. — `a_regular_expression_shows_each_match_with_its_groups`
592. Xpath finds nodes and values. — `xpath_finds_nodes_and_values`
593. A css selector finds elements in html that is not tidy. — `a_css_selector_finds_elements_in_html_that_is_not_tidy`
594. A document cannot read files or fetch anything. — `a_document_cannot_read_files_or_fetch_anything`
595. A regular expression that never finishes is stopped and the tester still works. — `a_regular_expression_that_never_finishes_is_stopped_and_the_tester_still_works`
596. Hostile expressions and texts are only text. — `hostile_expressions_and_texts_are_only_text`
597. Nothing is left behind and it is quick enough to use. — `nothing_is_left_behind_and_it_is_quick_enough_to_use`
598. Many tests at once do not get in each others way. — `many_tests_at_once_do_not_get_in_each_others_way`

### app (pure logic inside the window crate)

*crates/app/src/assets.rs*

599. Every category icon is bundled. — `every_category_icon_is_bundled`
600. Fonts are bundled. — `fonts_are_bundled`
601. Icons used by the ui exist. — `icons_used_by_the_ui_exist`
602. Missing assets return none. — `missing_assets_return_none`
*crates/app/src/chart.rs*

603. The scale is the largest value but at least one. — `the_scale_is_the_largest_value_but_at_least_one`
604. Points are spread across the width with zero at the bottom. — `points_are_spread_across_the_width_with_zero_at_the_bottom`
605. Values outside the scale stay inside the chart. — `values_outside_the_scale_stay_inside_the_chart`
*crates/app/src/clipboard.rs*

606. File names are made from element names. — `file_names_are_made_from_element_names`
*crates/app/src/combo.rs*

607. Selection is clamped to the options. — `selection_is_clamped_to_the_options`
*crates/app/src/curl_import.rs*

608. Clipboard text that looks like curl is recognised. — `clipboard_text_that_looks_like_curl_is_recognised`
*crates/app/src/expression.rs*

609. Extractors start the dialog with their expression. — `extractors_start_the_dialog_with_their_expression`
610. What was found is counted and listed. — `what_was_found_is_counted_and_listed`
*crates/app/src/har_import.rs*

611. Files that cannot be read as har text are refused with a reason. — `files_that_cannot_be_read_as_har_text_are_refused_with_a_reason`
*crates/app/src/palette_view.rs*

612. Empty messages. — `empty_messages`
*crates/app/src/results_view.rs*

613. Files that cannot be looked at are refused with a reason. — `files_that_cannot_be_looked_at_are_refused_with_a_reason`
*crates/app/src/run_props.rs*

614. A file inside the plans folder is written relative with slashes. — `a_file_inside_the_plans_folder_is_written_relative_with_slashes`
615. Only settings that name a file to read get a browse button. — `only_settings_that_name_a_file_to_read_get_a_browse_button`
*crates/app/src/running.rs*

616. A run starts as starting and becomes running on the first report. — `a_run_starts_as_starting_and_becomes_running_on_the_first_report`
617. Samples feed the numbers and lines the log. — `samples_feed_the_numbers_and_lines_the_log`
618. The log keeps only the latest lines. — `the_log_keeps_only_the_latest_lines`
619. How a run ends decides its outcome. — `how_a_run_ends_decides_its_outcome`
620. A failed run says why from what jmeter printed. — `a_failed_run_says_why_from_what_jmeter_printed`
621. Asking to stop moves to stopping and force is offered only after the grace time. — `asking_to_stop_moves_to_stopping_and_force_is_offered_only_after_the_grace_time`
622. Stopping something that is not running does nothing. — `stopping_something_that_is_not_running_does_nothing`
623. A stored run takes its final phase once its numbers are in. — `a_stored_run_takes_its_final_phase_once_its_numbers_are_in`
624. A stored run shows the length it had not the time since it was opened. — `a_stored_run_shows_the_length_it_had_not_the_time_since_it_was_opened`
625. Phases know whether the process is going. — `phases_know_whether_the_process_is_going`
*crates/app/src/shell.rs*

626. Shortcut conversion. — `shortcut_conversion`
627. Every shortcut is a valid keystroke. — `every_shortcut_is_a_valid_keystroke`
628. Display path strips the verbatim prefix. — `display_path_strips_the_verbatim_prefix`
629. Runtime check reports a missing java. — `runtime_check_reports_a_missing_java`
630. Runtime check reports a bad jmeter folder. — `runtime_check_reports_a_bad_jmeter_folder`
*crates/app/src/theme.rs*

631. Contrast math matches known values. — `contrast_math_matches_known_values`
632. Text is readable in both modes. — `text_is_readable_in_both_modes`
633. Colours that carry meaning stand out from the background. — `colours_that_carry_meaning_stand_out_from_the_background`
634. Dark is dark and light is light. — `dark_is_dark_and_light_is_light`
635. Surfaces are distinguishable from each other. — `surfaces_are_distinguishable_from_each_other`
636. Every category has a colour. — `every_category_has_a_colour`
637. Alpha helper only changes alpha. — `alpha_helper_only_changes_alpha`
638. Selection is translucent. — `selection_is_translucent`
*crates/app/src/think_times.rs*

639. Pauses are whole milliseconds within the limit. — `pauses_are_whole_milliseconds_within_the_limit`
*crates/app/src/ui_util.rs*

640. Highlight groups adjacent characters. — `highlight_groups_adjacent_characters`
641. Highlight without positions is one run. — `highlight_without_positions_is_one_run`
642. Highlight counts characters not bytes. — `highlight_counts_characters_not_bytes`
*crates/app/src/views_content.rs*

643. Java version is shortened. — `java_version_is_shortened`
*crates/app/src/views_run.rs*

644. The clock shows minutes and seconds and hours when needed. — `the_clock_shows_minutes_and_seconds_and_hours_when_needed`
645. Counts get thousands separators. — `counts_get_thousands_separators`
646. Rates are short. — `rates_are_short`

## 2. Whole-window tests (headless)

These run the real window without a screen, driving it with simulated keys, clicks and drags.

### Opening plans, tree, palette, tabs, panels, themes, settings

*crates/app/tests/shell.rs*

647. Starts empty without a plan. — `starts_empty_without_a_plan`
648. Opens a plan given at startup. — `opens_a_plan_given_at_startup`
649. A missing file shows an error and leaves the app usable. — `a_missing_file_shows_an_error_and_leaves_the_app_usable`
650. A file that is not a plan shows an error. — `a_file_that_is_not_a_plan_shows_an_error`
651. A hostile plan is refused with a message and no tab. — `a_hostile_plan_is_refused_with_a_message_and_no_tab`
652. Opening adds to recent files and failures remove them. — `opening_adds_to_recent_files_and_failures_remove_them`
653. Clicking a row redraws the editor and opens no tab. — `clicking_a_row_redraws_the_editor_and_opens_no_tab`
654. Every kind of element redraws the same tab. — `every_kind_of_element_redraws_the_same_tab`
655. Keyboard navigation in the tree. — `keyboard_navigation_in_the_tree`
656. Enter and space expand and collapse the selected element. — `enter_and_space_expand_and_collapse_the_selected_element`
657. Left and right collapse and expand. — `left_and_right_collapse_and_expand`
658. The chevron toggles without selecting. — `the_chevron_toggles_without_selecting`
659. Expand and collapse all from the palette. — `expand_and_collapse_all_from_the_palette`
660. Ctrl k opens and escape closes the palette. — `ctrl_k_opens_and_escape_closes_the_palette`
661. Clicking outside closes the palette. — `clicking_outside_closes_the_palette`
662. The title bar search box opens the palette. — `the_title_bar_search_box_opens_the_palette`
663. Palette filters as you type. — `palette_filters_as_you_type`
664. Goto element selects opens and focuses the tree. — `goto_element_selects_opens_and_focuses_the_tree`
665. Goto expands collapsed ancestors. — `goto_expands_collapsed_ancestors`
666. Arrow keys move through palette results. — `arrow_keys_move_through_palette_results`
667. Unavailable commands do nothing when chosen. — `unavailable_commands_do_nothing_when_chosen`
668. Add mode lists what fits under the selection. — `add_mode_lists_what_fits_under_the_selection`
669. The split editor is gone. — `the_split_editor_is_gone`
670. Opening the same plan again shows its tab. — `opening_the_same_plan_again_shows_its_tab`
671. Each plan has one tab and its own tree. — `each_plan_has_one_tab_and_its_own_tree`
672. Closing a plan tab closes the plan. — `closing_a_plan_tab_closes_the_plan`
673. Middle click closes a tab. — `middle_click_closes_a_tab`
674. Ctrl tab cycles through plans. — `ctrl_tab_cycles_through_plans`
675. Selecting an element brings the plan back from settings. — `selecting_an_element_brings_the_plan_back_from_settings`
676. The activity bar has no search item. — `the_activity_bar_has_no_search_item`
677. The inspector starts hidden and shows what can be added. — `the_inspector_starts_hidden_and_shows_what_can_be_added`
678. Can add follows the selection. — `can_add_follows_the_selection`
679. Panel shortcuts toggle and persist. — `panel_shortcuts_toggle_and_persist`
680. Dragging the sidebar handle resizes and saves. — `dragging_the_sidebar_handle_resizes_and_saves`
681. Theme commands switch and persist. — `theme_commands_switch_and_persist`
682. Settings tab opens and saves runtime paths. — `settings_tab_opens_and_saves_runtime_paths`
683. Settings tab theme buttons. — `settings_tab_theme_buttons`
684. About dialog opens from the palette and closes on click. — `about_dialog_opens_from_the_palette_and_closes_on_click`
685. Notices disappear by themselves. — `notices_disappear_by_themselves`
686. Reload keeps the selection. — `reload_keeps_the_selection`
687. Reload after the file is deleted shows an error and keeps the old plan. — `reload_after_the_file_is_deleted_shows_an_error_and_keeps_the_old_plan`
688. Recent plans appear in the palette and reopen. — `recent_plans_appear_in_the_palette_and_reopen`
689. A damaged settings file is reported at startup. — `a_damaged_settings_file_is_reported_at_startup`
690. Problems panel lists plan notes. — `problems_panel_lists_plan_notes`
691. Valid heap and interval are saved. — `valid_heap_and_interval_are_saved`
692. Invalid numbers are not saved and are reported. — `invalid_numbers_are_not_saved_and_are_reported`
693. A variable is not accepted for heap. — `a_variable_is_not_accepted_for_heap`
694. Enter in a number field applies. — `enter_in_a_number_field_applies`
695. The startup combo saves its choice. — `the_startup_combo_saves_its_choice`
696. Startup can reopen the last plan. — `startup_can_reopen_the_last_plan`
697. Startup skips a last plan that no longer exists. — `startup_skips_a_last_plan_that_no_longer_exists`
698. A plan on the command line wins over the startup setting. — `a_plan_on_the_command_line_wins_over_the_startup_setting`
699. Welcome screen stays when startup is welcome. — `welcome_screen_stays_when_startup_is_welcome`
700. The state file is never seen half written. — `the_state_file_is_never_seen_half_written`

### Editing: add, delete, move, drag and drop, undo, saving, recovery, scope, big plans

*crates/app/tests/editing.rs*

701. Adding from the palette puts the element under the selection and selects it. — `adding_from_the_palette_puts_the_element_under_the_selection_and_selects_it`
702. Adding with nothing selected goes under the test plan. — `adding_with_nothing_selected_goes_under_the_test_plan`
703. Clicking a name in can add adds it at once. — `clicking_a_name_in_can_add_adds_it_at_once`
704. Only allowed types can be added under the test plan. — `only_allowed_types_can_be_added_under_the_test_plan`
705. Delete removes the element selects a neighbour and undo brings it back. — `delete_removes_the_element_selects_a_neighbour_and_undo_brings_it_back`
706. The test plan cannot be deleted. — `the_test_plan_cannot_be_deleted`
707. Duplicate copies the element with its children. — `duplicate_copies_the_element_with_its_children`
708. Alt arrows move an element among its siblings. — `alt_arrows_move_an_element_among_its_siblings`
709. Ctrl t switches an element off and on. — `ctrl_t_switches_an_element_off_and_on`
710. Undo and redo walk through the history. — `undo_and_redo_walk_through_the_history`
711. A new edit after undo clears redo. — `a_new_edit_after_undo_clears_redo`
712. The edit commands are in the palette with their shortcuts. — `the_edit_commands_are_in_the_palette_with_their_shortcuts`
713. Ctrl s saves the edits to the file and clears the unsaved mark. — `ctrl_s_saves_the_edits_to_the_file_and_clears_the_unsaved_mark`
714. The recovery copy is written after a pause and removed on save. — `the_recovery_copy_is_written_after_a_pause_and_removed_on_save`
715. Only the latest edit writes the recovery copy. — `only_the_latest_edit_writes_the_recovery_copy`
716. Undoing back to the saved state removes the recovery copy. — `undoing_back_to_the_saved_state_removes_the_recovery_copy`
717. A recovery copy from an earlier session can be recovered. — `a_recovery_copy_from_an_earlier_session_can_be_recovered`
718. The recovery copy can be discarded. — `the_recovery_copy_can_be_discarded`
719. A copy that matches the file is ignored. — `a_copy_that_matches_the_file_is_ignored`
720. Closing a changed plan asks and cancel keeps it. — `closing_a_changed_plan_asks_and_cancel_keeps_it`
721. Dont save closes and drops the recovery copy. — `dont_save_closes_and_drops_the_recovery_copy`
722. Save in the question saves then closes. — `save_in_the_question_saves_then_closes`
723. Closing an unchanged plan does not ask. — `closing_an_unchanged_plan_does_not_ask`
724. Quitting with unsaved changes asks first. — `quitting_with_unsaved_changes_asks_first`
725. Close all asks once for every changed plan. — `close_all_asks_once_for_every_changed_plan`
726. Reloading a changed plan asks first. — `reloading_a_changed_plan_asks_first`
727. Each plan has its own history and unsaved mark. — `each_plan_has_its_own_history_and_unsaved_mark`
728. Save as writes a new file and renames the tab. — `save_as_writes_a_new_file_and_renames_the_tab`
729. Edits keep the selection and the tree in step. — `edits_keep_the_selection_and_the_tree_in_step`
730. Dropping onto an element makes it the last child. — `dropping_onto_an_element_makes_it_the_last_child`
731. Dropping above or below a row reorders. — `dropping_above_or_below_a_row_reorders`
732. Dropping below the last sibling moves to the end. — `dropping_below_the_last_sibling_moves_to_the_end`
733. A move the rules forbid is refused with a message. — `a_move_the_rules_forbid_is_refused_with_a_message`
734. The test plan cannot be dragged and nothing sits beside it. — `the_test_plan_cannot_be_dragged_and_nothing_sits_beside_it`
735. An element cannot be dropped into its own subtree. — `an_element_cannot_be_dropped_into_its_own_subtree`
736. The inspector lists what applies to a sampler and clicking selects it. — `the_inspector_lists_what_applies_to_a_sampler_and_clicking_selects_it`
737. The inspector shows what a config element applies to. — `the_inspector_shows_what_a_config_element_applies_to`
738. Scope follows edits. — `scope_follows_edits`
739. A plan of twenty thousand elements stays usable. — `a_plan_of_twenty_thousand_elements_stays_usable`

### Forms: boxes, numbers, drop-downs, tables, typing, hints, focus

*crates/app/tests/forms.rs*

740. A typed value is saved with enter and undone with undo. — `a_typed_value_is_saved_with_enter_and_undone_with_undo`
741. Leaving a box saves what was typed. — `leaving_a_box_saves_what_was_typed`
742. Tab moves to the next box and saves the one left. — `tab_moves_to_the_next_box_and_saves_the_one_left`
743. Escape puts the saved value back. — `escape_puts_the_saved_value_back`
744. The name box renames the element in the tree. — `the_name_box_renames_the_element_in_the_tree`
745. F2 puts the cursor in the name box. — `f2_puts_the_cursor_in_the_name_box`
746. The comment box edits the comments. — `the_comment_box_edits_the_comments`
747. Ctrl s saves text that was typed but not confirmed. — `ctrl_s_saves_text_that_was_typed_but_not_confirmed`
748. Number fields accept numbers and variables but not letters. — `number_fields_accept_numbers_and_variables_but_not_letters`
749. A variable in a number setting turns it into a string setting. — `a_variable_in_a_number_setting_turns_it_into_a_string_setting`
750. The loop count lives inside the main controller setting. — `the_loop_count_lives_inside_the_main_controller_setting`
751. Checkboxes toggle a setting and undo reverts. — `checkboxes_toggle_a_setting_and_undo_reverts`
752. A drop down saves the chosen value. — `a_drop_down_saves_the_chosen_value`
753. Undo updates the drop down. — `undo_updates_the_drop_down`
754. A value jmeter does not list is kept as a custom choice. — `a_value_jmeter_does_not_list_is_kept_as_a_custom_choice`
755. A setting made of several flags is edited flag by flag. — `a_setting_made_of_several_flags_is_edited_flag_by_flag`
756. Parameters are added edited and removed in a table. — `parameters_are_added_edited_and_removed_in_a_table`
757. Table changes undo in order. — `table_changes_undo_in_order`
758. User defined variables have their own table. — `user_defined_variables_have_their_own_table`
759. Assertion patterns are a list of strings. — `assertion_patterns_are_a_list_of_strings`
760. Elements without a hand made form get one from their settings. — `elements_without_a_hand_made_form_get_one_from_their_settings`
761. A script is edited in a text area. — `a_script_is_edited_in_a_text_area`
762. The test plan has a variables table and flags. — `the_test_plan_has_a_variables_table_and_flags`
763. Enabled button switches the element off. — `enabled_button_switches_the_element_off`
764. The form follows the selection. — `the_form_follows_the_selection`
765. A variable nothing defines shows up in the problems list. — `a_variable_nothing_defines_shows_up_in_the_problems_list`
766. The variable hints stay up to date with undo. — `the_variable_hints_stay_up_to_date_with_undo`
767. The box keeps the focus after enter even when the form changes shape. — `the_box_keeps_the_focus_after_enter_even_when_the_form_changes_shape`
768. Shortcuts work right after editing a box. — `shortcuts_work_right_after_editing_a_box`
769. Undo from a box keeps the window usable. — `undo_from_a_box_keeps_the_window_usable`
770. Typing marks the plan unsaved at once and deleting it again clears the mark. — `typing_marks_the_plan_unsaved_at_once_and_deleting_it_again_clears_the_mark`
771. Closing a tab asks about text that was only typed. — `closing_a_tab_asks_about_text_that_was_only_typed`
772. Typing in a table cell marks the plan at once. — `typing_in_a_table_cell_marks_the_plan_at_once`
773. Text typed in a box reaches the plan and the recovery copy after a pause. — `text_typed_in_a_box_reaches_the_plan_and_the_recovery_copy_after_a_pause`
774. Text typed in a table cell reaches the recovery copy. — `text_typed_in_a_table_cell_reaches_the_recovery_copy`
775. Half a number does not raise an error while typing. — `half_a_number_does_not_raise_an_error_while_typing`
776. The name box renames while typing. — `the_name_box_renames_while_typing`

### Text and number boxes

*crates/app/tests/fields.rs*

777. Typing and backspace. — `typing_and_backspace`
778. Caret movement and insertion. — `caret_movement_and_insertion`
779. Select all and replace. — `select_all_and_replace`
780. Shift arrows select and delete. — `shift_arrows_select_and_delete`
781. Word navigation and deletion. — `word_navigation_and_deletion`
782. Undo and redo. — `undo_and_redo`
783. Clipboard copy cut paste. — `clipboard_copy_cut_paste`
784. Pasting multiple lines gives one line. — `pasting_multiple_lines_gives_one_line`
785. Characters a plan cannot hold are dropped when typed or pasted. — `characters_a_plan_cannot_hold_are_dropped_when_typed_or_pasted`
786. Enter and escape emit events. — `enter_and_escape_emit_events`
787. Changed is emitted for every edit but not for navigation. — `changed_is_emitted_for_every_edit_but_not_for_navigation`
788. Number field rejects letters. — `number_field_rejects_letters`
789. Number field steps with arrow keys. — `number_field_steps_with_arrow_keys`
790. Number field accepts variables and does not step them. — `number_field_accepts_variables_and_does_not_step_them`
791. Number field reports problems. — `number_field_reports_problems`
792. Read only field ignores edits but allows copy. — `read_only_field_ignores_edits_but_allows_copy`
793. Set text replaces the content. — `set_text_replaces_the_content`
794. Typing unicode. — `typing_unicode`
795. Losing the focus is reported once. — `losing_the_focus_is_reported_once`

### Running plans: questions before a run, live numbers, stopping, run history, the runs folder, settings

*crates/app/tests/runs.rs*

796. Without java or jmeter the run buttons say why. — `without_java_or_jmeter_the_run_buttons_say_why`
797. The run menu and palette list every run command. — `the_run_menu_and_palette_list_every_run_command`
798. A plan with unsaved changes asks to save before running. — `a_plan_with_unsaved_changes_asks_to_save_before_running`
799. Saving to run writes the file then goes on to the next question. — `saving_to_run_writes_the_file_then_goes_on_to_the_next_question`
800. A runs folder that cannot be used stops the run and offers a way out. — `a_runs_folder_that_cannot_be_used_stops_the_run_and_offers_a_way_out`
801. Using the jmx folder from the question changes the setting and carries on. — `using_the_jmx_folder_from_the_question_changes_the_setting_and_carries_on`
802. A custom folder that is a file is refused. — `a_custom_folder_that_is_a_file_is_refused`
803. A plan with scripts asks once for that plan file. — `a_plan_with_scripts_asks_once_for_that_plan_file`
804. A plan without scripts is never asked about them. — `a_plan_without_scripts_is_never_asked_about_them`
805. The settings tab chooses where run files go. — `the_settings_tab_chooses_where_run_files_go`
806. The number of runs kept is a checked number. — `the_number_of_runs_kept_is_a_checked_number`
807. Lowering the number kept removes the older runs of the open plan at once. — `lowering_the_number_kept_removes_the_older_runs_of_the_open_plan_at_once`
808. The history tab lists the runs of the open plan newest first. — `the_history_tab_lists_the_runs_of_the_open_plan_newest_first`
809. History shows runs of the plan in the tab not every plan. — `history_shows_runs_of_the_plan_in_the_tab_not_every_plan`
810. Clicking a stored run shows its numbers and charts. — `clicking_a_stored_run_shows_its_numbers_and_charts`
811. A damaged results file still opens as a stored run. — `a_damaged_results_file_still_opens_as_a_stored_run`
812. A run that was cut short is shown as interrupted. — `a_run_that_was_cut_short_is_shown_as_interrupted`
813. Removing one run asks first and removes only that run. — `removing_one_run_asks_first_and_removes_only_that_run`
814. Removing never touches a folder that is not a run. — `removing_never_touches_a_folder_that_is_not_a_run`
815. Clearing the shared runs folder asks and removes only runs. — `clearing_the_shared_runs_folder_asks_and_removes_only_runs`
816. Clear is not available for the runs next to the plan. — `clear_is_not_available_for_the_runs_next_to_the_plan`
817. Clearing an empty shared folder says so without asking. — `clearing_an_empty_shared_folder_says_so_without_asking`
818. F5 runs the saved plan and shows the live numbers. — `f5_runs_the_saved_plan_and_shows_the_live_numbers`
819. The run header says where the files are and the status bar follows the run. — `the_run_header_says_where_the_files_are_and_the_status_bar_follows_the_run`
820. Stop ends a run that would never end and it is kept as stopped. — `stop_ends_a_run_that_would_never_end_and_it_is_kept_as_stopped`
821. Stop now and ending jmeter by force both work. — `stop_now_and_ending_jmeter_by_force_both_work`
822. The chosen folder gets a runs folder with a folder for each run. — `the_chosen_folder_gets_a_runs_folder_with_a_folder_for_each_run`
823. Only the newest runs are kept after a run and their files stay. — `only_the_newest_runs_are_kept_after_a_run_and_their_files_stay`
824. Validate runs the whole plan once with one user. — `validate_runs_the_whole_plan_once_with_one_user`
825. Send once sends the selected request and only that. — `send_once_sends_the_selected_request_and_only_that`
826. Send once is not available on something that is not a request. — `send_once_is_not_available_on_something_that_is_not_a_request`
827. Quitting during a run asks and cancelling leaves the run going. — `quitting_during_a_run_asks_and_cancelling_leaves_the_run_going`
828. A plan jmeter cannot read ends as failed with a reason. — `a_plan_jmeter_cannot_read_ends_as_failed_with_a_reason`
829. A run with a flood of samples keeps the window responsive. — `a_run_with_a_flood_of_samples_keeps_the_window_responsive`
830. Viewing a large stored run is quick and does not block the window. — `viewing_a_large_stored_run_is_quick_and_does_not_block_the_window`
831. A run keeps the failed requests and the viewer shows what was sent and received. — `a_run_keeps_the_failed_requests_and_the_viewer_shows_what_was_sent_and_received`
832. All requests can be kept and walked through with the arrow keys. — `all_requests_can_be_kept_and_walked_through_with_the_arrow_keys`
833. With details off nothing is kept and the viewer says how to turn it on. — `with_details_off_nothing_is_kept_and_the_viewer_says_how_to_turn_it_on`
834. Send once opens the viewer on the answer. — `send_once_opens_the_viewer_on_the_answer`
835. The viewer is not available before a run. — `the_viewer_is_not_available_before_a_run`
836. A stored run loads its details and counts damaged lines. — `a_stored_run_loads_its_details_and_counts_damaged_lines`
837. A huge or hostile details file cannot hang or flood the window. — `a_huge_or_hostile_details_file_cannot_hang_or_flood_the_window`
838. The variables of a request are listed and a request without any says so. — `the_variables_of_a_request_are_listed_and_a_request_without_any_says_so`
839. The response of a request can be tried with an expression. — `the_response_of_a_request_can_be_tried_with_an_expression`
840. After a real run an expression can be tried on what came back. — `after_a_real_run_an_expression_can_be_tried_on_what_came_back`
841. A results file opens as a run with its numbers and charts. — `a_results_file_opens_as_a_run_with_its_numbers_and_charts`
842. Files that are not results say so and change nothing. — `files_that_are_not_results_say_so_and_change_nothing`
843. Opening a large results file is quick and the window stays responsive. — `opening_a_large_results_file_is_quick_and_the_window_stays_responsive`
844. The html report is made once and then opened. — `the_html_report_is_made_once_and_then_opened`
845. A results file opened on its own gets its report next to it. — `a_results_file_opened_on_its_own_gets_its_report_next_to_it`
846. A report that cannot be made says why and leaves nothing. — `a_report_that_cannot_be_made_says_why_and_leaves_nothing`
847. A report without java or jmeter says so. — `a_report_without_java_or_jmeter_says_so`
848. The run properties dialog keeps the lines for this plan. — `the_run_properties_dialog_keeps_the_lines_for_this_plan`
849. Wrong lines are explained and nothing is saved. — `wrong_lines_are_explained_and_nothing_is_saved`
850. Properties can be loaded from a file and hostile files are refused. — `properties_can_be_loaded_from_a_file_and_hostile_files_are_refused`
851. The properties reach the plan in a run a check and a single request. — `the_properties_reach_the_plan_in_a_run_a_check_and_a_single_request`
852. Properties that became invalid stop the run with a reason. — `properties_that_became_invalid_stop_the_run_with_a_reason`
853. A chosen file is written in the form relative when it is next to the plan. — `a_chosen_file_is_written_in_the_form_relative_when_it_is_next_to_the_plan`
854. The per request table has a total row and scrolls instead of cutting columns. — `the_per_request_table_has_a_total_row_and_scrolls_instead_of_cutting_columns`
855. Another element opens at the top of its form. — `another_element_opens_at_the_top_of_its_form`

## 3. Real-window checks

These start the built app, drive it with real mouse and keyboard input, and read its state file and the files it writes (`tests/ui/smoke.ps1`).


### 1. Open a plan, select by mouse, navigate by keyboard

856. Plan loads. — `smoke.ps1`, section "1. Open a plan, select by mouse, navigate by keyboard"
857. Tree shows 15 rows. — `smoke.ps1`, section "1. Open a plan, select by mouse, navigate by keyboard"
858. Clicking a row selects it. — `smoke.ps1`, section "1. Open a plan, select by mouse, navigate by keyboard"
859. And the editor shows it. — `smoke.ps1`, section "1. Open a plan, select by mouse, navigate by keyboard"
860. Without opening a tab. — `smoke.ps1`, section "1. Open a plan, select by mouse, navigate by keyboard"
861. Down moves the selection and the editor follows. — `smoke.ps1`, section "1. Open a plan, select by mouse, navigate by keyboard"
862. End selects the last row. — `smoke.ps1`, section "1. Open a plan, select by mouse, navigate by keyboard"

### 2. Command palette

863. Ctrl+K opens the palette. — `smoke.ps1`, section "2. Command palette"
864. Typing filters commands. — `smoke.ps1`, section "2. Command palette"
865. Escape closes it. — `smoke.ps1`, section "2. Command palette"
866. Go to element selects it. — `smoke.ps1`, section "2. Command palette"
867. And the editor shows it, still in one tab. — `smoke.ps1`, section "2. Command palette"

### 3. Inspector

868. The inspector starts hidden. — `smoke.ps1`, section "3. Inspector"
869. Ctrl+Alt+B shows it. — `smoke.ps1`, section "3. Inspector"
870. An empty title bar click changes nothing. — `smoke.ps1`, section "3. Inspector"

### 4. Menus

871. Ctrl+J shows the results panel. — `smoke.ps1`, section "4. Menus"
872. Ctrl+B hides the test plan panel. — `smoke.ps1`, section "4. Menus"

### 5. Themes

873. Light theme applies. — `smoke.ps1`, section "5. Themes"
874. Dark theme applies. — `smoke.ps1`, section "5. Themes"

### 6. Resizing a panel and remembering it

875. Dragging the handle widens the panel. — `smoke.ps1`, section "6. Resizing a panel and remembering it"

### 7. Window controls and remembered size

876. Maximize fills the screen. — `smoke.ps1`, section "7. Window controls and remembered size"
877. Close button ends the app. — `smoke.ps1`, section "7. Window controls and remembered size"
878. Window state is saved. — `smoke.ps1`, section "7. Window controls and remembered size"

### 8. Settings tab and drop-down

879. Ctrl+, opens settings. — `smoke.ps1`, section "8. Settings tab and drop-down"
880. Choosing from the drop-down with the keyboard saves it. — `smoke.ps1`, section "8. Settings tab and drop-down"
881. Choosing with the mouse saves it. — `smoke.ps1`, section "8. Settings tab and drop-down"
882. An invalid heap size is rejected. — `smoke.ps1`, section "8. Settings tab and drop-down"
883. A valid heap size is saved. — `smoke.ps1`, section "8. Settings tab and drop-down"

### 9. Open dialog

884. Ctrl+O opens a file dialog and loads the chosen plan. — `smoke.ps1`, section "9. Open dialog"
885. A second plan gets its own tab. — `smoke.ps1`, section "9. Open dialog"
886. And the tree shows the second plan. — `smoke.ps1`, section "9. Open dialog"
887. Ctrl+Tab switches back to the first plan. — `smoke.ps1`, section "9. Open dialog"

### 10. Editing: forms, add, delete, undo, drag, save, recovery

888. Go to element shows its form. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
889. Typing in a box and pressing Enter changes the plan. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
890. Clicking a checkbox is another change. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
891. Ctrl+Z undoes the last change. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
892. Ctrl+S saves. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
893. The file keeps its original formatting elsewhere. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
894. Adding from the palette selects the new element. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
895. Delete removes the selected element. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
896. And undo brings it back. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
897. Dragging a row onto another moves the element. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
898. A recovery copy is written next to the plan after a pause. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
899. Opening the plan again offers the unsaved work. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
900. Recovering brings the changes back, unsaved. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
901. Closing the window with unsaved changes asks first. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
902. Cancel keeps the app open with the work. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
903. Quitting with unsaved changes asks first. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
904. Don't Save quits. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
905. And removes the recovery copy. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"

### 11. Running: F5, live numbers, history, Validate, Send once

906. Java and JMeter are found in the background. — `smoke.ps1`, section "11. Running: F5, live numbers, history, Validate, Send once"
907. F5 starts the run and opens the lower panel on the Run tab. — `smoke.ps1`, section "11. Running: F5, live numbers, history, Validate, Send once"
908. The run finishes. — `smoke.ps1`, section "11. Running: F5, live numbers, history, Validate, Send once"
909. All 12 samples were counted, 6 of them errors. — `smoke.ps1`, section "11. Running: F5, live numbers, history, Validate, Send once"
910. The plan file was not changed by the run. — `smoke.ps1`, section "11. Running: F5, live numbers, history, Validate, Send once"
911. The run has its own folder inside runs next to the plan. — `smoke.ps1`, section "11. Running: F5, live numbers, history, Validate, Send once"
912. With results, log, the plan as run and run.json. — `smoke.ps1`, section "11. Running: F5, live numbers, history, Validate, Send once"
913. The history lists the run. — `smoke.ps1`, section "11. Running: F5, live numbers, history, Validate, Send once"
914. The six failed requests were kept with what was sent and received. — `smoke.ps1`, section "11. Running: F5, live numbers, history, Validate, Send once"
915. Ctrl+Shift+R opens the request viewer on the first failure. — `smoke.ps1`, section "11. Running: F5, live numbers, history, Validate, Send once"
916. It shows the request that was sent and the answer that came back. — `smoke.ps1`, section "11. Running: F5, live numbers, history, Validate, Send once"
917. The Down key moves to the next request. — `smoke.ps1`, section "11. Running: F5, live numbers, history, Validate, Send once"
918. Ctrl+Shift+T opens the expression dialog with the response of the chosen request. — `smoke.ps1`, section "11. Running: F5, live numbers, history, Validate, Send once"
919. The real JMeter libraries answer, and say that the text is not JSON. — `smoke.ps1`, section "11. Running: F5, live numbers, history, Validate, Send once"
920. Escape closes the dialog. — `smoke.ps1`, section "11. Running: F5, live numbers, history, Validate, Send once"
921. Ctrl+F5 validates: one user once, so each request once. — `smoke.ps1`, section "11. Running: F5, live numbers, history, Validate, Send once"
922. F6 sends the selected request once. — `smoke.ps1`, section "11. Running: F5, live numbers, history, Validate, Send once"
923. All three runs are in the history. — `smoke.ps1`, section "11. Running: F5, live numbers, history, Validate, Send once"
924. Ctrl+Shift+H makes the HTML report with JMeter's generator and opens its page. — `smoke.ps1`, section "11. Running: F5, live numbers, history, Validate, Send once"

### 12. New plan

925. Ctrl+N asks which kind of plan to start. — `smoke.ps1`, section "12. New plan"

### 13. Text area for a script

926. The script element is selected. — `smoke.ps1`, section "13. Text area for a script"
927. Typing in the area marks the plan as changed. — `smoke.ps1`, section "13. Text area for a script"
928. The saved script keeps its lines. — `smoke.ps1`, section "13. Text area for a script"
929. And nothing is left to save. — `smoke.ps1`, section "13. Text area for a script"

### 14. Copy and paste of elements

930. A request is selected in the tree. — `smoke.ps1`, section "14. Copy and paste of elements"
931. Ctrl+C then Ctrl+V adds a copy with what is under it. — `smoke.ps1`, section "14. Copy and paste of elements"
932. One Ctrl+Z takes the whole paste away. — `smoke.ps1`, section "14. Copy and paste of elements"
933. Ctrl+X removes the element. — `smoke.ps1`, section "14. Copy and paste of elements"

### 15. Import from cURL

934. Ctrl+Shift+I opens the cURL dialog. — `smoke.ps1`, section "15. Import from cURL"
935. Typing goes into the dialog's text area. — `smoke.ps1`, section "15. Import from cURL"

### 16. Run properties

936. Ctrl+Shift+J opens the run properties dialog. — `smoke.ps1`, section "16. Run properties"
937. Typing goes into the properties box. — `smoke.ps1`, section "16. Run properties"
938. Escape closes it without saving. — `smoke.ps1`, section "16. Run properties"

Total scenarios listed: 938.
