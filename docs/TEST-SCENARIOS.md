# Test scenarios

Every automated test and real-window check that exists today, one scenario per line. The name of the test is shown in `code font`.

## What should work

- **Plans open and save safely.** Any `.jmx` file opens, and a saved file keeps every untouched byte exactly as it was. Missing or invalid files give a clear message, not a crash.
- **Every change can be undone.** Add, delete, duplicate, move, rename, enable or disable, and every setting change undo and redo. Undoing everything gives back the original file.
- **Elements follow JMeter's rules.** Only elements JMeter allows can be added or dropped where you choose. A plan with one of every element loads in real JMeter.
- **Forms edit everything.** Every element has a form. Text, numbers (including `${variables}`), checkboxes, drop-downs, flag groups and tables all save to the plan, and typing shows as unsaved at once.
- **Work is never lost silently.** Unsaved marks, a recovery copy while you work, and a question before closing, quitting or reloading.
- **The window behaves.** One tab per plan, the tree drives the editor, and the palette, shortcuts, drag and drop and right-click menu work. The keyboard focus stays where you are typing.
- **The extras are correct.** Scope & Order matches how JMeter runs things, and `${variables}` that nothing defines are flagged.
- **Settings and looks persist.** Themes, panel sizes, window size, recent files and the Java and JMeter paths are remembered, and a damaged settings file is set aside.
- **Big plans stay usable.** A 20,000-element plan opens and edits quickly.

The right-click menu can only be tested in the real window (section 3), because the headless window cannot open pop-up menus.

## How to run each kind

- Unit tests and whole-window tests: `cargo test --workspace --exclude gpui-spike --exclude zed-ui-spike`
- Real-window checks: `cargo build -p jmeter-ui`, then `powershell -ExecutionPolicy Bypass -File tests\ui\smoke.ps1`
- Some unit tests read sample plans from the JMeter install (set `JMX_CORPUS` / `JMETER_HOME`); they skip themselves when it is missing.

## 1. Unit tests

These test the logic without a window.

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
*crates/jmx-model/tests/runcopy.rs*

24. Injects one collector under the test plan. — `injects_one_collector_under_the_test_plan`
25. The path is escaped. — `the_path_is_escaped`
26. Everything before the insertion point is unchanged. — `everything_before_the_insertion_point_is_unchanged`
27. Rejects documents without a test plan. — `rejects_documents_without_a_test_plan`
28. Sets loops everywhere and counts. — `sets_loops_everywhere_and_counts`
*crates/jmx-model/tests/xml.rs*

29. Unchanged documents are written back exactly. — `unchanged_documents_are_written_back_exactly`
30. Attributes are decoded on read. — `attributes_are_decoded_on_read`
31. Editing an attribute rewrites only that start tag. — `editing_an_attribute_rewrites_only_that_start_tag`
32. Adding a new attribute. — `adding_a_new_attribute`
33. Set text escapes and expands self closing elements. — `set_text_escapes_and_expands_self_closing_elements`
34. Text reading joins runs and decodes. — `text_reading_joins_runs_and_decodes`
35. Pushing children into a self closing element. — `pushing_children_into_a_self_closing_element`
36. Walking visits every element depth first. — `walking_visits_every_element_depth_first`
37. Elements skips text and comments. — `elements_skips_text_and_comments`
38. Malformed input is an error not a panic. — `malformed_input_is_an_error_not_a_panic`
39. Escape and unescape are inverse. — `escape_and_unescape_are_inverse`
40. Unknown entities are left alone. — `unknown_entities_are_left_alone`
41. Deeply nested documents work. — `deeply_nested_documents_work`
42. Fragments parse to one element. — `fragments_parse_to_one_element`

### plan-core (elements, rules, editing, forms, scope, variables, saving)

*crates/plan-core/src/catalog.rs*

43. Table is sorted and unique. — `table_is_sorted_and_unique`
44. Core elements. — `core_elements`
45. Shared tags use the gui class. — `shared_tags_use_the_gui_class`
46. Plugin classes are guessed from their names. — `plugin_classes_are_guessed_from_their_names`
47. Unknown core tags get a readable name. — `unknown_core_tags_get_a_readable_name`
48. Humanize handles acronyms. — `humanize_handles_acronyms`
49. Every category has an icon and labels. — `every_category_has_an_icon_and_labels`
50. List is complete and unique. — `list_is_complete_and_unique`
51. Variants are expanded. — `variants_are_expanded`
52. Every quick add entry exists. — `every_quick_add_entry_exists`
53. Sorted by category then name. — `sorted_by_category_then_name`
54. Elements missing from the table are found through jmeters menus. — `elements_missing_from_the_table_are_found_through_jmeters_menus`
55. Every menu element is classified the same way by both sources. — `every_menu_element_is_classified_the_same_way_by_both_sources`
*crates/plan-core/src/edit.rs*

56. A fresh plan is clean and writes back unchanged. — `a_fresh_plan_is_clean_and_writes_back_unchanged`
57. Adding puts the element last and gives it jmeters defaults. — `adding_puts_the_element_last_and_gives_it_jmeters_defaults`
58. The rules decide what can be added where. — `the_rules_decide_what_can_be_added_where`
59. Undo and redo restore the text exactly. — `undo_and_redo_restore_the_text_exactly`
60. Removing and undoing keeps the same ids. — `removing_and_undoing_keeps_the_same_ids`
61. The test plan cannot be removed. — `the_test_plan_cannot_be_removed`
62. Duplicate copies children and sits after the original. — `duplicate_copies_children_and_sits_after_the_original`
63. Moving reorders and reparents and undoes. — `moving_reorders_and_reparents_and_undoes`
64. Moving between levels re indents and undoes exactly. — `moving_between_levels_re_indents_and_undoes_exactly`
65. Settings changes are undoable and a no op is not recorded. — `settings_changes_are_undoable_and_a_no_op_is_not_recorded`
66. Disabling shows in the plan. — `disabling_shows_in_the_plan`
67. Dirty follows saves and history. — `dirty_follows_saves_and_history`
68. A new edit clears redo. — `a_new_edit_clears_redo`
69. Missing elements are errors. — `missing_elements_are_errors`
*crates/plan-core/src/files.rs*

70. The recovery copy sits next to the plan. — `the_recovery_copy_sits_next_to_the_plan`
71. Save replaces the file and removes the recovery copy. — `save_replaces_the_file_and_removes_the_recovery_copy`
72. A failed save leaves the old file. — `a_failed_save_leaves_the_old_file`
73. A leftover copy is found when it differs. — `a_leftover_copy_is_found_when_it_differs`
74. A copy identical to the file is not a recovery. — `a_copy_identical_to_the_file_is_not_a_recovery`
75. A copy without its plan is still offered. — `a_copy_without_its_plan_is_still_offered`
76. An older copy is marked as not newer. — `an_older_copy_is_marked_as_not_newer`
*crates/plan-core/src/forms.rs*

77. Every curated form names settings that jmeter knows. — `every_curated_form_names_settings_that_jmeter_knows`
78. Forms exist for the everyday elements. — `forms_exist_for_the_everyday_elements`
79. Curated forms cover most of jmeters menus. — `curated_forms_cover_most_of_jmeters_menus`
80. Field properties are unique within a form except for flags in one setting. — `field_properties_are_unique_within_a_form_except_for_flags_in_one_setting`
81. Choices include the default and have unique values. — `choices_include_the_default_and_have_unique_values`
82. Every table path exists in a new element or is known. — `every_table_path_exists_in_a_new_element_or_is_known`
83. The generic form covers every simple setting. — `the_generic_form_covers_every_simple_setting`
84. Labels are made readable. — `labels_are_made_readable`
85. Java hash matches java. — `java_hash_matches_java`
86. Rows of every shape parse and escape. — `rows_of_every_shape_parse_and_escape`
87. Parameters can be added edited removed and undone. — `parameters_can_be_added_edited_removed_and_undone`
88. Response assertion patterns are a list of strings. — `response_assertion_patterns_are_a_list_of_strings`
89. Bad tables are errors. — `bad_tables_are_errors`
*crates/plan-core/src/fuzzy.rs*

90. Empty query keeps everything in order. — `empty_query_keeps_everything_in_order`
91. Matches subsequences and reports positions. — `matches_subsequences_and_reports_positions`
92. Better matches rank first. — `better_matches_rank_first`
93. Lower case ignores case and upper case does not. — `lower_case_ignores_case_and_upper_case_does_not`
94. Exact prefix beats scattered match. — `exact_prefix_beats_scattered_match`
95. Ties keep input order. — `ties_keep_input_order`
*crates/plan-core/src/model.rs*

96. Builds the tree in document order. — `builds_the_tree_in_document_order`
97. Empty names fall back to the type label. — `empty_names_fall_back_to_the_type_label`
98. Parents children and paths. — `parents_children_and_paths`
99. Categories flags and summary. — `categories_flags_and_summary`
100. Move rules and cycles. — `move_rules_and_cycles`
101. Rejects malformed plans. — `rejects_malformed_plans`
102. Missing file error names the path. — `missing_file_error_names_the_path`
103. Props and comments are carried over. — `props_and_comments_are_carried_over`
*crates/plan-core/src/problems.rs*

104. A healthy plan has no problems. — `a_healthy_plan_has_no_problems`
105. No thread group. — `no_thread_group`
106. Thread group without requests. — `thread_group_without_requests`
107. Requests nested in controllers count. — `requests_nested_in_controllers_count`
108. Disabled thread group is a note not a warning. — `disabled_thread_group_is_a_note_not_a_warning`
109. Plugin and unknown elements are grouped by class. — `plugin_and_unknown_elements_are_grouped_by_class`
110. Disabled elements are counted. — `disabled_elements_are_counted`
111. Empty plan gives nothing. — `empty_plan_gives_nothing`
*crates/plan-core/src/rules.rs*

112. Test plan and workbench are never children. — `test_plan_and_workbench_are_never_children`
113. Thread groups and fragments only under the plan. — `thread_groups_and_fragments_only_under_the_plan`
114. Non test elements go under plan fragment or workbench. — `non_test_elements_go_under_plan_fragment_or_workbench`
115. Plan accepts config timers listeners but not samplers or controllers. — `plan_accepts_config_timers_listeners_but_not_samplers_or_controllers`
116. Controllers and thread groups accept samplers and controllers. — `controllers_and_thread_groups_accept_samplers_and_controllers`
117. Workbench accepts everything except thread groups and structure. — `workbench_accepts_everything_except_thread_groups_and_structure`
118. Samplers accept processors but not samplers or controllers. — `samplers_accept_processors_but_not_samplers_or_controllers`
119. Recorder accepts processors but not samplers. — `recorder_accepts_processors_but_not_samplers`
120. Leaf categories accept nothing. — `leaf_categories_accept_nothing`
121. Unknown parent is permissive but still blocks structural children. — `unknown_parent_is_permissive_but_still_blocks_structural_children`
122. Allowed children lists. — `allowed_children_lists`
*crates/plan-core/src/scope.rs*

123. A sampler gets its steps in jmeters order. — `a_sampler_gets_its_steps_in_jmeters_order`
124. Configuration is nearest first and the others follow their note. — `configuration_is_nearest_first_and_the_others_follow_their_note`
125. An element under a sibling sampler does not apply. — `an_element_under_a_sibling_sampler_does_not_apply`
126. Something under a sampler applies to that sampler only. — `something_under_a_sampler_applies_to_that_sampler_only`
127. Something under a controller applies to every sampler inside its parent. — `something_under_a_controller_applies_to_every_sampler_inside_its_parent`
128. A sampler applies to itself and a controller to nothing in particular. — `a_sampler_applies_to_itself_and_a_controller_to_nothing_in_particular`
129. A controller lists what it shares with everything inside. — `a_controller_lists_what_it_shares_with_everything_inside`
130. Switched off elements are listed but inactive. — `switched_off_elements_are_listed_but_inactive`
131. Nothing runs inside a test fragment. — `nothing_runs_inside_a_test_fragment`
132. Non samplers have no run order. — `non_samplers_have_no_run_order`
*crates/plan-core/src/search.rs*

133. Finds by name. — `finds_by_name`
134. Finds by type when the name does not match. — `finds_by_type_when_the_name_does_not_match`
135. Name matches rank above type matches. — `name_matches_rank_above_type_matches`
136. Empty query and limit. — `empty_query_and_limit`
*crates/plan-core/src/templates.rs*

137. Every menu entry loads. — `every_menu_entry_loads`
138. Entries are unique by tag and gui. — `entries_are_unique_by_tag_and_gui`
139. Menu groups match the catalog. — `menu_groups_match_the_catalog`
140. Http request has what the gui gives it. — `http_request_has_what_the_gui_gives_it`
141. Listeners with the same tag are told apart by gui. — `listeners_with_the_same_tag_are_told_apart_by_gui`
*crates/plan-core/src/tree.rs*

142. Collapsed tree shows only roots. — `collapsed_tree_shows_only_roots`
143. Expanded to depth. — `expanded_to_depth`
144. Toggle and row flags. — `toggle_and_row_flags`
145. Select expands ancestors. — `select_expands_ancestors`
146. Collapsing an ancestor moves the selection to it. — `collapsing_an_ancestor_moves_the_selection_to_it`
147. Expand all and collapse all. — `expand_all_and_collapse_all`
148. Arrow navigation stops at the ends. — `arrow_navigation_stops_at_the_ends`
149. Up with no selection picks the last row and pages clamp. — `up_with_no_selection_picks_the_last_row_and_pages_clamp`
150. Left and right keys. — `left_and_right_keys`
151. Selecting a missing node is ignored. — `selecting_a_missing_node_is_ignored`
*crates/plan-core/src/variables.rs*

152. Names come from settings tables and the test plan. — `names_come_from_settings_tables_and_the_test_plan`
153. Scripts can define variables. — `scripts_can_define_variables`
154. Only undefined names are reported once per element. — `only_undefined_names_are_reported_once_per_element`
155. What an extractor adds to a name counts as defined. — `what_an_extractor_adds_to_a_name_counts_as_defined`
156. Functions and odd text are left alone. — `functions_and_odd_text_are_left_alone`
157. Built in names are known. — `built_in_names_are_known`
158. A plan in which everything is defined has no hints. — `a_plan_in_which_everything_is_defined_has_no_hints`
*crates/plan-core/tests/corpus.rs*

159. Simple jmeter sample. — `simple_jmeter_sample`
160. Web test template has the usual structure. — `web_test_template_has_the_usual_structure`
161. Plugin plan keeps plugin elements. — `plugin_plan_keeps_plugin_elements`
162. Every real parent child pair obeys the rules. — `every_real_parent_child_pair_obeys_the_rules`
163. Every core tag in the catalog has a label and icon. — `every_core_tag_in_the_catalog_has_a_label_and_icon`
164. A very large plan loads and expands quickly. — `a_very_large_plan_loads_and_expands_quickly`
*crates/plan-core/tests/edit_corpus.rs*

165. Random edits on real plans stay valid and undo exactly. — `random_edits_on_real_plans_stay_valid_and_undo_exactly`
166. Editing a large plan is fast enough. — `editing_a_large_plan_is_fast_enough`
*crates/plan-core/tests/jmeter_load.rs*

167. A plan with every element loads in jmeter. — `a_plan_with_every_element_loads_in_jmeter`

### shell-core (settings, layout, text editing, palette, actions)

*crates/shell-core/src/actions.rs*

168. All lists every action once. — `all_lists_every_action_once`
169. Shortcuts are unique. — `shortcuts_are_unique`
170. Nothing open. — `nothing_open`
171. With a plan and tabs. — `with_a_plan_and_tabs`
172. Next tab needs two tabs. — `next_tab_needs_two_tabs`
173. Run and stop follow the run state. — `run_and_stop_follow_the_run_state`
174. Saving needs unsaved changes and save as only a plan. — `saving_needs_unsaved_changes_and_save_as_only_a_plan`
175. Undo and redo follow the history. — `undo_and_redo_follow_the_history`
176. Element commands need a selection the test plan is special. — `element_commands_need_a_selection_the_test_plan_is_special`
177. Every group has actions. — `every_group_has_actions`
178. Adding follows the edit capability. — `adding_follows_the_edit_capability`
*crates/shell-core/src/layout.rs*

179. Starts empty with panels from settings. — `starts_empty_with_panels_from_settings`
180. The inspector is hidden by default. — `the_inspector_is_hidden_by_default`
181. Opening a tab places it after the active one. — `opening_a_tab_places_it_after_the_active_one`
182. A plan never gets a second tab. — `a_plan_never_gets_a_second_tab`
183. Closing activates the right neighbour then the left. — `closing_activates_the_right_neighbour_then_the_left`
184. Closing a tab before the active one keeps it active. — `closing_a_tab_before_the_active_one_keeps_it_active`
185. Next and previous wrap. — `next_and_previous_wrap`
186. Close others and close all. — `close_others_and_close_all`
187. Rename and dirty apply to the matching tab only. — `rename_and_dirty_apply_to_the_matching_tab_only`
188. Panel sizes clamp and ignore nan. — `panel_sizes_clamp_and_ignore_nan`
189. Toggles and saving back to settings. — `toggles_and_saving_back_to_settings`
*crates/shell-core/src/number.rs*

190. Parses numbers. — `parses_numbers`
191. Rejects bad input with a reason. — `rejects_bad_input_with_a_reason`
192. Variables and functions are valid. — `variables_and_functions_are_valid`
193. Empty is allowed only when optional. — `empty_is_allowed_only_when_optional`
194. Partial input is accepted while typing. — `partial_input_is_accepted_while_typing`
195. Partial expressions are accepted. — `partial_expressions_are_accepted`
196. Stepping. — `stepping`
197. Expressions and junk are not stepped. — `expressions_and_junk_are_not_stepped`
198. Formatting. — `formatting`
199. Clamp handles open ranges. — `clamp_handles_open_ranges`
*crates/shell-core/src/palette.rs*

200. Prefixes pick the mode. — `prefixes_pick_the_mode`
201. Commands mode lists only actions. — `commands_mode_lists_only_actions`
202. Unavailable commands sink to the bottom with a reason. — `unavailable_commands_sink_to_the_bottom_with_a_reason`
203. Elements mode finds by name and carries the type. — `elements_mode_finds_by_name_and_carries_the_type`
204. Elements mode without a query lists the tree in order. — `elements_mode_without_a_query_lists_the_tree_in_order`
205. Plain queries mix commands recents and elements. — `plain_queries_mix_commands_recents_and_elements`
206. An empty query shows commands and a few recents but no elements. — `an_empty_query_shows_commands_and_a_few_recents_but_no_elements`
207. Recent items use the file name as the title. — `recent_items_use_the_file_name_as_the_title`
208. Add mode offers only what fits under the selection. — `add_mode_offers_only_what_fits_under_the_selection`
209. Add mode with a query also lists what does not fit as unavailable. — `add_mode_with_a_query_also_lists_what_does_not_fit_as_unavailable`
210. Add mode quick list comes first without a query. — `add_mode_quick_list_comes_first_without_a_query`
211. Add mode without a selection uses the test plan. — `add_mode_without_a_selection_uses_the_test_plan`
212. Add mode under a leaf offers nothing. — `add_mode_under_a_leaf_offers_nothing`
213. Add items are unavailable until editing exists. — `add_items_are_unavailable_until_editing_exists`
214. Limit is applied after ranking. — `limit_is_applied_after_ranking`
215. Shortcuts are included for commands. — `shortcuts_are_included_for_commands`
*crates/shell-core/src/settings.rs*

216. Missing file gives defaults without a notice. — `missing_file_gives_defaults_without_a_notice`
217. Round trip. — `round_trip`
218. Older files with missing fields still load. — `older_files_with_missing_fields_still_load`
219. Unknown fields are ignored. — `unknown_fields_are_ignored`
220. Damaged file is set aside. — `damaged_file_is_set_aside`
221. Wrong types count as damage. — `wrong_types_count_as_damage`
222. Sizes are clamped on load and save. — `sizes_are_clamped_on_load_and_save`
223. Recent files are unique ordered and capped. — `recent_files_are_unique_ordered_and_capped`
224. Default path ends in the app folder. — `default_path_ends_in_the_app_folder`
*crates/shell-core/src/text_buffer.rs*

225. Typing appends at the caret. — `typing_appends_at_the_caret`
226. Inserting nothing changes nothing. — `inserting_nothing_changes_nothing`
227. Typing replaces the selection. — `typing_replaces_the_selection`
228. Backspace and delete. — `backspace_and_delete`
229. Selection is deleted as a whole. — `selection_is_deleted_as_a_whole`
230. Arrows collapse a selection to its edge. — `arrows_collapse_a_selection_to_its_edge`
231. Shift arrows extend in either direction. — `shift_arrows_extend_in_either_direction`
232. Movement stops at the ends. — `movement_stops_at_the_ends`
233. Words. — `words`
234. Delete by word. — `delete_by_word`
235. Double click selects a word. — `double_click_selects_a_word`
236. Graphemes are never split. — `graphemes_are_never_split`
237. Positions past the end are clamped. — `positions_past_the_end_are_clamped`
238. Newlines become spaces. — `newlines_become_spaces`
239. Filter and length limit. — `filter_and_length_limit`
240. Typing in a row is one undo step. — `typing_in_a_row_is_one_undo_step`
241. Moving the caret ends the typing run. — `moving_the_caret_ends_the_typing_run`
242. Paste cut and delete are separate steps. — `paste_cut_and_delete_are_separate_steps`
243. New edits clear redo. — `new_edits_clear_redo`
244. Backspaces in a row coalesce. — `backspaces_in_a_row_coalesce`
245. Copy needs a selection. — `copy_needs_a_selection`
246. Replace range for input methods. — `replace_range_for_input_methods`
247. Set text resets history. — `set_text_resets_history`
248. Undo history is bounded. — `undo_history_is_bounded`
*crates/shell-core/tests/settings_run.rs*

249. Defaults match jmeters own. — `defaults_match_jmeters_own`
250. Values round trip. — `values_round_trip`
251. Startup is stored as a word. — `startup_is_stored_as_a_word`
252. Out of range values are clamped. — `out_of_range_values_are_clamped`
253. An unknown startup word counts as damage. — `an_unknown_startup_word_counts_as_damage`
254. Old files without the new fields get defaults. — `old_files_without_the_new_fields_get_defaults`

### jvm-runtime (finding Java and JMeter, results parsing)

*crates/jvm-runtime/src/locate.rs*

255. Java versions. — `java_versions`
*crates/jvm-runtime/src/results.rs*

256. Summary interval line. — `summary_interval_line`
257. Summary cumulative line has no threads. — `summary_cumulative_line_has_no_threads`
258. Summary rate when elapsed is zero. — `summary_rate_when_elapsed_is_zero`
259. Stop port line. — `stop_port_line`
260. Jtl tail handles partial lines and quotes. — `jtl_tail_handles_partial_lines_and_quotes`

### app (pure logic inside the window crate)

*crates/app/src/assets.rs*

261. Every category icon is bundled. — `every_category_icon_is_bundled`
262. Fonts are bundled. — `fonts_are_bundled`
263. Icons used by the ui exist. — `icons_used_by_the_ui_exist`
264. Missing assets return none. — `missing_assets_return_none`
*crates/app/src/combo.rs*

265. Selection is clamped to the options. — `selection_is_clamped_to_the_options`
*crates/app/src/palette_view.rs*

266. Empty messages. — `empty_messages`
*crates/app/src/shell.rs*

267. Shortcut conversion. — `shortcut_conversion`
268. Every shortcut is a valid keystroke. — `every_shortcut_is_a_valid_keystroke`
269. Display path strips the verbatim prefix. — `display_path_strips_the_verbatim_prefix`
270. Runtime check reports a missing java. — `runtime_check_reports_a_missing_java`
271. Runtime check reports a bad jmeter folder. — `runtime_check_reports_a_bad_jmeter_folder`
*crates/app/src/theme.rs*

272. Contrast math matches known values. — `contrast_math_matches_known_values`
273. Text is readable in both modes. — `text_is_readable_in_both_modes`
274. Colours that carry meaning stand out from the background. — `colours_that_carry_meaning_stand_out_from_the_background`
275. Dark is dark and light is light. — `dark_is_dark_and_light_is_light`
276. Surfaces are distinguishable from each other. — `surfaces_are_distinguishable_from_each_other`
277. Every category has a colour. — `every_category_has_a_colour`
278. Alpha helper only changes alpha. — `alpha_helper_only_changes_alpha`
279. Selection is translucent. — `selection_is_translucent`
*crates/app/src/ui_util.rs*

280. Highlight groups adjacent characters. — `highlight_groups_adjacent_characters`
281. Highlight without positions is one run. — `highlight_without_positions_is_one_run`
282. Highlight counts characters not bytes. — `highlight_counts_characters_not_bytes`
*crates/app/src/views_content.rs*

283. Java version is shortened. — `java_version_is_shortened`

## 2. Whole-window tests (headless)

These run the real window without a screen, driving it with simulated keys, clicks and drags.

### Opening plans, tree, palette, tabs, panels, themes, settings

*crates/app/tests/shell.rs*

284. Starts empty without a plan. — `starts_empty_without_a_plan`
285. Opens a plan given at startup. — `opens_a_plan_given_at_startup`
286. A missing file shows an error and leaves the app usable. — `a_missing_file_shows_an_error_and_leaves_the_app_usable`
287. A file that is not a plan shows an error. — `a_file_that_is_not_a_plan_shows_an_error`
288. Opening adds to recent files and failures remove them. — `opening_adds_to_recent_files_and_failures_remove_them`
289. Clicking a row redraws the editor and opens no tab. — `clicking_a_row_redraws_the_editor_and_opens_no_tab`
290. Every kind of element redraws the same tab. — `every_kind_of_element_redraws_the_same_tab`
291. Keyboard navigation in the tree. — `keyboard_navigation_in_the_tree`
292. Enter and space expand and collapse the selected element. — `enter_and_space_expand_and_collapse_the_selected_element`
293. Left and right collapse and expand. — `left_and_right_collapse_and_expand`
294. The chevron toggles without selecting. — `the_chevron_toggles_without_selecting`
295. Expand and collapse all from the palette. — `expand_and_collapse_all_from_the_palette`
296. Ctrl k opens and escape closes the palette. — `ctrl_k_opens_and_escape_closes_the_palette`
297. Clicking outside closes the palette. — `clicking_outside_closes_the_palette`
298. The title bar search box opens the palette. — `the_title_bar_search_box_opens_the_palette`
299. Palette filters as you type. — `palette_filters_as_you_type`
300. Goto element selects opens and focuses the tree. — `goto_element_selects_opens_and_focuses_the_tree`
301. Goto expands collapsed ancestors. — `goto_expands_collapsed_ancestors`
302. Arrow keys move through palette results. — `arrow_keys_move_through_palette_results`
303. Unavailable commands do nothing when chosen. — `unavailable_commands_do_nothing_when_chosen`
304. Add mode lists what fits under the selection. — `add_mode_lists_what_fits_under_the_selection`
305. The split editor is gone. — `the_split_editor_is_gone`
306. Opening the same plan again shows its tab. — `opening_the_same_plan_again_shows_its_tab`
307. Each plan has one tab and its own tree. — `each_plan_has_one_tab_and_its_own_tree`
308. Closing a plan tab closes the plan. — `closing_a_plan_tab_closes_the_plan`
309. Middle click closes a tab. — `middle_click_closes_a_tab`
310. Ctrl tab cycles through plans. — `ctrl_tab_cycles_through_plans`
311. Selecting an element brings the plan back from settings. — `selecting_an_element_brings_the_plan_back_from_settings`
312. The activity bar has no search item. — `the_activity_bar_has_no_search_item`
313. The inspector starts hidden and shows what can be added. — `the_inspector_starts_hidden_and_shows_what_can_be_added`
314. Can add follows the selection. — `can_add_follows_the_selection`
315. Panel shortcuts toggle and persist. — `panel_shortcuts_toggle_and_persist`
316. Dragging the sidebar handle resizes and saves. — `dragging_the_sidebar_handle_resizes_and_saves`
317. Theme commands switch and persist. — `theme_commands_switch_and_persist`
318. Settings tab opens and saves runtime paths. — `settings_tab_opens_and_saves_runtime_paths`
319. Settings tab theme buttons. — `settings_tab_theme_buttons`
320. About dialog opens from the palette and closes on click. — `about_dialog_opens_from_the_palette_and_closes_on_click`
321. Notices disappear by themselves. — `notices_disappear_by_themselves`
322. Reload keeps the selection. — `reload_keeps_the_selection`
323. Reload after the file is deleted shows an error and keeps the old plan. — `reload_after_the_file_is_deleted_shows_an_error_and_keeps_the_old_plan`
324. Recent plans appear in the palette and reopen. — `recent_plans_appear_in_the_palette_and_reopen`
325. A damaged settings file is reported at startup. — `a_damaged_settings_file_is_reported_at_startup`
326. Problems panel lists plan notes. — `problems_panel_lists_plan_notes`
327. Valid heap and interval are saved. — `valid_heap_and_interval_are_saved`
328. Invalid numbers are not saved and are reported. — `invalid_numbers_are_not_saved_and_are_reported`
329. A variable is not accepted for heap. — `a_variable_is_not_accepted_for_heap`
330. Enter in a number field applies. — `enter_in_a_number_field_applies`
331. The startup combo saves its choice. — `the_startup_combo_saves_its_choice`
332. Startup can reopen the last plan. — `startup_can_reopen_the_last_plan`
333. Startup skips a last plan that no longer exists. — `startup_skips_a_last_plan_that_no_longer_exists`
334. A plan on the command line wins over the startup setting. — `a_plan_on_the_command_line_wins_over_the_startup_setting`
335. Welcome screen stays when startup is welcome. — `welcome_screen_stays_when_startup_is_welcome`

### Editing: add, delete, move, drag and drop, undo, saving, recovery, scope, big plans

*crates/app/tests/editing.rs*

336. Adding from the palette puts the element under the selection and selects it. — `adding_from_the_palette_puts_the_element_under_the_selection_and_selects_it`
337. Adding with nothing selected goes under the test plan. — `adding_with_nothing_selected_goes_under_the_test_plan`
338. Clicking a name in can add adds it at once. — `clicking_a_name_in_can_add_adds_it_at_once`
339. Only allowed types can be added under the test plan. — `only_allowed_types_can_be_added_under_the_test_plan`
340. Delete removes the element selects a neighbour and undo brings it back. — `delete_removes_the_element_selects_a_neighbour_and_undo_brings_it_back`
341. The test plan cannot be deleted. — `the_test_plan_cannot_be_deleted`
342. Duplicate copies the element with its children. — `duplicate_copies_the_element_with_its_children`
343. Alt arrows move an element among its siblings. — `alt_arrows_move_an_element_among_its_siblings`
344. Ctrl t switches an element off and on. — `ctrl_t_switches_an_element_off_and_on`
345. Undo and redo walk through the history. — `undo_and_redo_walk_through_the_history`
346. A new edit after undo clears redo. — `a_new_edit_after_undo_clears_redo`
347. The edit commands are in the palette with their shortcuts. — `the_edit_commands_are_in_the_palette_with_their_shortcuts`
348. Ctrl s saves the edits to the file and clears the unsaved mark. — `ctrl_s_saves_the_edits_to_the_file_and_clears_the_unsaved_mark`
349. The recovery copy is written after a pause and removed on save. — `the_recovery_copy_is_written_after_a_pause_and_removed_on_save`
350. Only the latest edit writes the recovery copy. — `only_the_latest_edit_writes_the_recovery_copy`
351. Undoing back to the saved state removes the recovery copy. — `undoing_back_to_the_saved_state_removes_the_recovery_copy`
352. A recovery copy from an earlier session can be recovered. — `a_recovery_copy_from_an_earlier_session_can_be_recovered`
353. The recovery copy can be discarded. — `the_recovery_copy_can_be_discarded`
354. A copy that matches the file is ignored. — `a_copy_that_matches_the_file_is_ignored`
355. Closing a changed plan asks and cancel keeps it. — `closing_a_changed_plan_asks_and_cancel_keeps_it`
356. Dont save closes and drops the recovery copy. — `dont_save_closes_and_drops_the_recovery_copy`
357. Save in the question saves then closes. — `save_in_the_question_saves_then_closes`
358. Closing an unchanged plan does not ask. — `closing_an_unchanged_plan_does_not_ask`
359. Quitting with unsaved changes asks first. — `quitting_with_unsaved_changes_asks_first`
360. Close all asks once for every changed plan. — `close_all_asks_once_for_every_changed_plan`
361. Reloading a changed plan asks first. — `reloading_a_changed_plan_asks_first`
362. Each plan has its own history and unsaved mark. — `each_plan_has_its_own_history_and_unsaved_mark`
363. Save as writes a new file and renames the tab. — `save_as_writes_a_new_file_and_renames_the_tab`
364. Edits keep the selection and the tree in step. — `edits_keep_the_selection_and_the_tree_in_step`
365. Dropping onto an element makes it the last child. — `dropping_onto_an_element_makes_it_the_last_child`
366. Dropping above or below a row reorders. — `dropping_above_or_below_a_row_reorders`
367. Dropping below the last sibling moves to the end. — `dropping_below_the_last_sibling_moves_to_the_end`
368. A move the rules forbid is refused with a message. — `a_move_the_rules_forbid_is_refused_with_a_message`
369. The test plan cannot be dragged and nothing sits beside it. — `the_test_plan_cannot_be_dragged_and_nothing_sits_beside_it`
370. An element cannot be dropped into its own subtree. — `an_element_cannot_be_dropped_into_its_own_subtree`
371. The inspector lists what applies to a sampler and clicking selects it. — `the_inspector_lists_what_applies_to_a_sampler_and_clicking_selects_it`
372. The inspector shows what a config element applies to. — `the_inspector_shows_what_a_config_element_applies_to`
373. Scope follows edits. — `scope_follows_edits`
374. A plan of twenty thousand elements stays usable. — `a_plan_of_twenty_thousand_elements_stays_usable`

### Forms: boxes, numbers, drop-downs, tables, typing, hints, focus

*crates/app/tests/forms.rs*

375. A typed value is saved with enter and undone with undo. — `a_typed_value_is_saved_with_enter_and_undone_with_undo`
376. Leaving a box saves what was typed. — `leaving_a_box_saves_what_was_typed`
377. Tab moves to the next box and saves the one left. — `tab_moves_to_the_next_box_and_saves_the_one_left`
378. Escape puts the saved value back. — `escape_puts_the_saved_value_back`
379. The name box renames the element in the tree. — `the_name_box_renames_the_element_in_the_tree`
380. F2 puts the cursor in the name box. — `f2_puts_the_cursor_in_the_name_box`
381. The comment box edits the comments. — `the_comment_box_edits_the_comments`
382. Ctrl s saves text that was typed but not confirmed. — `ctrl_s_saves_text_that_was_typed_but_not_confirmed`
383. Number fields accept numbers and variables but not letters. — `number_fields_accept_numbers_and_variables_but_not_letters`
384. A variable in a number setting turns it into a string setting. — `a_variable_in_a_number_setting_turns_it_into_a_string_setting`
385. The loop count lives inside the main controller setting. — `the_loop_count_lives_inside_the_main_controller_setting`
386. Checkboxes toggle a setting and undo reverts. — `checkboxes_toggle_a_setting_and_undo_reverts`
387. A drop down saves the chosen value. — `a_drop_down_saves_the_chosen_value`
388. Undo updates the drop down. — `undo_updates_the_drop_down`
389. A value jmeter does not list is kept as a custom choice. — `a_value_jmeter_does_not_list_is_kept_as_a_custom_choice`
390. A setting made of several flags is edited flag by flag. — `a_setting_made_of_several_flags_is_edited_flag_by_flag`
391. Parameters are added edited and removed in a table. — `parameters_are_added_edited_and_removed_in_a_table`
392. Table changes undo in order. — `table_changes_undo_in_order`
393. User defined variables have their own table. — `user_defined_variables_have_their_own_table`
394. Assertion patterns are a list of strings. — `assertion_patterns_are_a_list_of_strings`
395. Elements without a hand made form get one from their settings. — `elements_without_a_hand_made_form_get_one_from_their_settings`
396. Long text is shown but not edited. — `long_text_is_shown_but_not_edited`
397. The test plan has a variables table and flags. — `the_test_plan_has_a_variables_table_and_flags`
398. Enabled button switches the element off. — `enabled_button_switches_the_element_off`
399. The form follows the selection. — `the_form_follows_the_selection`
400. A variable nothing defines shows up in the problems list. — `a_variable_nothing_defines_shows_up_in_the_problems_list`
401. The variable hints stay up to date with undo. — `the_variable_hints_stay_up_to_date_with_undo`
402. The box keeps the focus after enter even when the form changes shape. — `the_box_keeps_the_focus_after_enter_even_when_the_form_changes_shape`
403. Shortcuts work right after editing a box. — `shortcuts_work_right_after_editing_a_box`
404. Undo from a box keeps the window usable. — `undo_from_a_box_keeps_the_window_usable`
405. Typing marks the plan unsaved at once and deleting it again clears the mark. — `typing_marks_the_plan_unsaved_at_once_and_deleting_it_again_clears_the_mark`
406. Closing a tab asks about text that was only typed. — `closing_a_tab_asks_about_text_that_was_only_typed`
407. Typing in a table cell marks the plan at once. — `typing_in_a_table_cell_marks_the_plan_at_once`
408. Text typed in a box reaches the plan and the recovery copy after a pause. — `text_typed_in_a_box_reaches_the_plan_and_the_recovery_copy_after_a_pause`
409. Text typed in a table cell reaches the recovery copy. — `text_typed_in_a_table_cell_reaches_the_recovery_copy`
410. Half a number does not raise an error while typing. — `half_a_number_does_not_raise_an_error_while_typing`
411. The name box renames while typing. — `the_name_box_renames_while_typing`

### Text and number boxes

*crates/app/tests/fields.rs*

412. Typing and backspace. — `typing_and_backspace`
413. Caret movement and insertion. — `caret_movement_and_insertion`
414. Select all and replace. — `select_all_and_replace`
415. Shift arrows select and delete. — `shift_arrows_select_and_delete`
416. Word navigation and deletion. — `word_navigation_and_deletion`
417. Undo and redo. — `undo_and_redo`
418. Clipboard copy cut paste. — `clipboard_copy_cut_paste`
419. Pasting multiple lines gives one line. — `pasting_multiple_lines_gives_one_line`
420. Enter and escape emit events. — `enter_and_escape_emit_events`
421. Changed is emitted for every edit but not for navigation. — `changed_is_emitted_for_every_edit_but_not_for_navigation`
422. Number field rejects letters. — `number_field_rejects_letters`
423. Number field steps with arrow keys. — `number_field_steps_with_arrow_keys`
424. Number field accepts variables and does not step them. — `number_field_accepts_variables_and_does_not_step_them`
425. Number field reports problems. — `number_field_reports_problems`
426. Read only field ignores edits but allows copy. — `read_only_field_ignores_edits_but_allows_copy`
427. Set text replaces the content. — `set_text_replaces_the_content`
428. Typing unicode. — `typing_unicode`
429. Losing the focus is reported once. — `losing_the_focus_is_reported_once`

## 3. Real-window checks

These start the built app, drive it with real mouse and keyboard input, and read its state file and the files it writes (`tests/ui/smoke.ps1`).


### 1. Open a plan, select by mouse, navigate by keyboard

430. Plan loads. — `smoke.ps1`, section "1. Open a plan, select by mouse, navigate by keyboard"
431. Tree shows 15 rows. — `smoke.ps1`, section "1. Open a plan, select by mouse, navigate by keyboard"
432. Clicking a row selects it. — `smoke.ps1`, section "1. Open a plan, select by mouse, navigate by keyboard"
433. And the editor shows it. — `smoke.ps1`, section "1. Open a plan, select by mouse, navigate by keyboard"
434. Without opening a tab. — `smoke.ps1`, section "1. Open a plan, select by mouse, navigate by keyboard"
435. Down moves the selection and the editor follows. — `smoke.ps1`, section "1. Open a plan, select by mouse, navigate by keyboard"
436. End selects the last row. — `smoke.ps1`, section "1. Open a plan, select by mouse, navigate by keyboard"

### 2. Command palette

437. Ctrl+K opens the palette. — `smoke.ps1`, section "2. Command palette"
438. Typing filters commands. — `smoke.ps1`, section "2. Command palette"
439. Escape closes it. — `smoke.ps1`, section "2. Command palette"
440. Go to element selects it. — `smoke.ps1`, section "2. Command palette"
441. And the editor shows it, still in one tab. — `smoke.ps1`, section "2. Command palette"

### 3. Inspector

442. The inspector starts hidden. — `smoke.ps1`, section "3. Inspector"
443. Ctrl+Alt+B shows it. — `smoke.ps1`, section "3. Inspector"
444. An empty title bar click changes nothing. — `smoke.ps1`, section "3. Inspector"

### 4. Menus

445. Ctrl+J shows the results panel. — `smoke.ps1`, section "4. Menus"
446. Ctrl+B hides the test plan panel. — `smoke.ps1`, section "4. Menus"

### 5. Themes

447. Light theme applies. — `smoke.ps1`, section "5. Themes"
448. Dark theme applies. — `smoke.ps1`, section "5. Themes"

### 6. Resizing a panel and remembering it

449. Dragging the handle widens the panel. — `smoke.ps1`, section "6. Resizing a panel and remembering it"

### 7. Window controls and remembered size

450. Maximize fills the screen. — `smoke.ps1`, section "7. Window controls and remembered size"
451. Close button ends the app. — `smoke.ps1`, section "7. Window controls and remembered size"
452. Window state is saved. — `smoke.ps1`, section "7. Window controls and remembered size"

### 8. Settings tab and drop-down

453. Ctrl+, opens settings. — `smoke.ps1`, section "8. Settings tab and drop-down"
454. Choosing from the drop-down with the keyboard saves it. — `smoke.ps1`, section "8. Settings tab and drop-down"
455. Choosing with the mouse saves it. — `smoke.ps1`, section "8. Settings tab and drop-down"
456. An invalid heap size is rejected. — `smoke.ps1`, section "8. Settings tab and drop-down"
457. A valid heap size is saved. — `smoke.ps1`, section "8. Settings tab and drop-down"

### 9. Open dialog

458. Ctrl+O opens a file dialog and loads the chosen plan. — `smoke.ps1`, section "9. Open dialog"
459. A second plan gets its own tab. — `smoke.ps1`, section "9. Open dialog"
460. And the tree shows the second plan. — `smoke.ps1`, section "9. Open dialog"
461. Ctrl+Tab switches back to the first plan. — `smoke.ps1`, section "9. Open dialog"

### 10. Editing: forms, add, delete, undo, drag, save, recovery

462. Go to element shows its form. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
463. Typing in a box and pressing Enter changes the plan. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
464. Clicking a checkbox is another change. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
465. Ctrl+Z undoes the last change. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
466. Ctrl+S saves. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
467. The file keeps its original formatting elsewhere. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
468. Adding from the palette selects the new element. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
469. Delete removes the selected element. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
470. And undo brings it back. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
471. Dragging a row onto another moves the element. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
472. A recovery copy is written next to the plan after a pause. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
473. Opening the plan again offers the unsaved work. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
474. Recovering brings the changes back, unsaved. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
475. Closing the window with unsaved changes asks first. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
476. Cancel keeps the app open with the work. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
477. Quitting with unsaved changes asks first. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
478. Don't Save quits. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"
479. And removes the recovery copy. — `smoke.ps1`, section "10. Editing: forms, add, delete, undo, drag, save, recovery"

Total scenarios listed: 479.
