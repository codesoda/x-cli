# Read-only X protocol research

Observed **2026-09-10 UTC**. Research only: public HTTP source/metadata retrieval and static inspection; no Bird installation/execution, browser-profile access, Keychain access, guest activation, or authenticated API requests. No session credentials or public bearer value are reproduced. Local history was unavailable (`ctx status`: generation verification failure); no history repair attempted.

## Conclusions

- Current public X JavaScript supplies concrete read-query definitions, including **`Viewer` for authenticated identity**. These are source-verified, **not live authenticated interoperability tests**.
- Prefer `TweetResultByRestId` for a single post, `TweetDetail` for conversation context. Do not infer complete reply trees.
- Bird is useful historical reference, not a current protocol authority or required dependency. **One Bird search fallback ID is currently explicitly a mutation in X's bundle. Never copy its fallback list.**
- macOS Chrome's inspected current Chromium implementation still supports Keychain-derived `v10` AES-CBC. Cookie DB schema 24 requires verifying the decrypted host digest. This does not establish compatibility with every released Chrome channel/profile or grant access through OS protection.

## 1. Provenance and license

Canonical author/project: **Peter Steinberger, `steipete/bird`, npm `@steipete/bird`**. The author's [profile README at `7f8fd020b386013f35256cb3c94c8809a97c37da`](https://github.com/steipete/steipete/blob/7f8fd020b386013f35256cb3c94c8809a97c37da/README.md) links `steipete/bird` and says “had to make it private.” The unauthenticated [repository API](https://api.github.com/repos/steipete/bird) returned 404; 404 alone cannot distinguish private/deleted repositories.

Public release examined:

- [npm metadata](https://registry.npmjs.org/@steipete/bird/0.8.0): version **0.8.0**, published **2026-01-19T07:17:30.221Z**; recorded `gitHead` **`6ae239383c0692b0dc96ea07de42d5b17d6ee2a3`**. Git source at that revision was not independently accessible/verified. Metadata now marks the package deprecated/unsupported and does not supply a license field.
- [Exact package archive](https://registry.npmjs.org/@steipete/bird/-/bird-0.8.0.tgz): fetched and inspected in memory, never executed/extracted as an installation. SHA-1 verified against registry: **`a32c34074c2799646b07bb8b25b379d59f57b870`**. Registry integrity: `sha512-p7+a9a/olzf1Rxe56a51VMFoBlFQpFVosC5B8dB3rOT8UbSZ3Ey5eXCZoLDjKXDf8xKINvSmGtMAd3yjeE4Gcw==` (metadata; not independently signature-verified).
- Archive `package/LICENSE` explicitly says **MIT License, Copyright (c) 2025 Peter Steinberger**, with the standard permission/notice/disclaimer text. [Version-pinned readable copy](https://unpkg.com/@steipete/bird@0.8.0/LICENSE). Preserve the copyright and permission notice if copying substantial code; audit dependencies separately. Missing npm license metadata is not evidence that the shipped MIT license is absent.
- [jawond/bird at `c0d08f32352fd8e0063d69c9b57760ff4c940b11`](https://github.com/jawond/bird/tree/c0d08f32352fd8e0063d69c9b57760ff4c940b11) is an older public copy: inspected default-head commit dated 2025-12-05. Its public availability does **not** make it the canonical current upstream or prove equality with npm 0.8.0. `steipete/birdclaw` is a separate archive-oriented project.

Historical reference files below are version-pinned published JavaScript, not a claimed reconstruction of inaccessible TypeScript:

- [B-base](https://unpkg.com/@steipete/bird@0.8.0/dist/lib/twitter-client-base.js), [B-constants](https://unpkg.com/@steipete/bird@0.8.0/dist/lib/twitter-client-constants.js), [B-IDs](https://unpkg.com/@steipete/bird@0.8.0/dist/lib/query-ids.json), [B-discovery](https://unpkg.com/@steipete/bird@0.8.0/dist/lib/runtime-query-ids.js).
- [B-identity](https://unpkg.com/@steipete/bird@0.8.0/dist/lib/twitter-client-users.js), [B-user](https://unpkg.com/@steipete/bird@0.8.0/dist/lib/twitter-client-user-lookup.js), [B-timeline](https://unpkg.com/@steipete/bird@0.8.0/dist/lib/twitter-client-user-tweets.js).
- [B-detail](https://unpkg.com/@steipete/bird@0.8.0/dist/lib/twitter-client-tweet-detail.js), [B-search](https://unpkg.com/@steipete/bird@0.8.0/dist/lib/twitter-client-search.js), [B-features](https://unpkg.com/@steipete/bird@0.8.0/dist/lib/twitter-client-features.js), [B-parsing](https://unpkg.com/@steipete/bird@0.8.0/dist/lib/twitter-client-utils.js).

## 2. Current first-party evidence

Public `https://x.com/explore`, `/notifications`, and `/settings/profile` returned logged-out HTML advertising **[X-main](https://abs.twimg.com/responsive-web/client-web/main.7313c8670b523249a.js)**. This is the primary source for all “current” definitions below:

- 1,122,841 bytes; SHA-256 **`f2c17811723ec0617ef6cf213fd56eb732d4f042c7ff81b7d72e33778f9ead65`**.
- `/` and `/?lang=en` instead advertised a different frontend: [entry-client-logged-out-BTHi9EQM.js](https://abs.twimg.com/x-web/x-web/entry-client-logged-out-BTHi9EQM.js), 22,043 bytes; SHA-256 **`917dde1567aa44f0b8c28f0db235f28037f31b3bea1cd15bb4dd070fd413b543`**. No target operation definitions were found in that entry file (its entire dependency graph was not searched).
- Consequence: a homepage-only, old-asset-path discovery regex is insufficient. Availability to this unauthenticated observer is not proof that every account/region receives the same rollout.

### Transport and identity

X-main's GraphQL adapter uses **GET for `operationType:"query"` unless explicitly forced to POST**, with URL-encoded JSON `variables`, `features`, and optional `fieldToggles`:

`https://x.com/i/api/graphql/<queryId>/<operationName>`

The adapter selects only declared feature-switch names and serializes each as `getValueWithoutScribeImpression(name) === true`; field toggles are filtered to the operation's declared names. Do not copy all features globally or interpret a listed switch as necessarily enabled.

**Concrete identity endpoint:**

`GET https://x.com/i/api/graphql/9t128XgFic52jPUEkJMf6w/Viewer`

Current X-main `fetchViewer` variables: `{withCommunitiesMemberships: <c9s_enabled boolean>}`. Field toggles supplied by the call site: `{isDelegate: <caller boolean>, withAuxiliaryUserLabels: <blue_business_multi_affiliates_ui_enabled boolean>}`. For the initial non-delegated account model, propose `isDelegate:false`; do not infer delegated identity support. Response: **`data.viewer.user_results.result`**, requiring `__typename:"User"`, nonempty string `rest_id`, and `core.screen_name` (legacy compatibility may check `legacy.screen_name`). Pin the stable `rest_id`, not the handle or GraphQL opaque `id`.

B-identity historically tries `https://x.com/i/api/account/settings.json`, then API-domain settings, then `https://x.com/i/api/account/verify_credentials.json?skip_status=true&include_entities=false` and `https://api.twitter.com/1.1/account/verify_credentials.json?skip_status=true&include_entities=false`. These are **historical candidates, not confirmed working identity endpoints**. X-main contains `account/settings` but not `verify_credentials`. Bird's parser accepts `user_id`, `user_id_str`, or nested `user.id_str`/string `user.id`, but notably **does not accept top-level `id_str`** from a conventional verify-credentials user object. Do not transplant this parser or its authenticated-settings-HTML scraping fallback. `UserByScreenName` verifies the requested profile, not the cookie owner's identity.

B-base documents the historical session envelope: the `auth_token` and `ct0` cookies, `x-csrf-token` matching that same session's `ct0`, authorization with the public web-client bearer, `x-twitter-auth-type: OAuth2Session`, active-user/client-language headers, origin/referer. These cookies are account-level authority, not read-only credentials. The public bearer alone is not user authentication. Browser-style additional headers and transaction-ID requirements remain an interoperability blocker: Bird generates random transaction bytes, whereas X-main loads its transaction implementation from a dynamic chunk. Random IDs are not established as valid current behavior. No authentication envelope was exercised here.

### Operation manifest

All six current definitions below declare **`operationType:"query"`** in X-main. Variables are statically observed call-site shapes, not a server-validated minimal schema. Omit optional cursor/context fields when absent. Retain IDs as strings.

| Operation / module | Current query ID | Variables and raw JSON response root |
|---|---|---|
| `Viewer` / 227618 | `9t128XgFic52jPUEkJMf6w` | See identity section; `data.viewer.user_results.result` |
| `TweetResultByRestId` / 728926 | `snmujSvB_9WXyd8yjvZ24Q` | Single-post no-replies branch: `{tweetId, withCommunity:false, includePromotedContent:false, withVoice:false}`; `data.tweetResult.result` |
| `TweetDetail` / 720832 | `FyR-GrebyjdkRoW1z6uCgQ` | `{focalTweetId, cursor?, referrer?, controller_data?, rux_context?, with_rux_injections?, rankingMode:"Relevance", includePromotedContent:true, withCommunity:<c9s_enabled>, withQuickPromoteEligibilityTweetFields:true, withBirdwatchNotes:<responsive_web_birdwatch_consumption_enabled>, withVoice:<voice_consumption_enabled>, isReaderMode?}`; `data.threaded_conversation_with_injections_v2.instructions` |
| `SearchTimeline` / 447423 | `KPSo2_UWdOMpPJwjhfT1Qg` | `{rawQuery, count, cursor?, querySource, product, withGrokTranslatedBio:<Top/People and translation switch>, withQuickPromoteEligibilityTweetFields:<caller === true>}`; `data.search_by_raw_query.search_timeline.timeline.instructions`. `product:"Latest"`, `querySource:"typed_query"`, count 20 are Bird's historical read-search choices. |
| `UserByScreenName` / 360985 | `KybxDj9RrADIITXlGG8kpw` | `{screen_name, withGrokTranslatedBio:<responsive_web_grok_bio_auto_translation_is_enabled>}`; `data.user.result`, then `rest_id`, `core.screen_name`, `core.name`; handle `UserUnavailable` |
| `UserTweets` / 859881 | `OeFjWKHutsuyWXZGmLr02A` | `{userId, count, cursor?, sortByMostLiked?, includePromotedContent:true, withQuickPromoteEligibilityTweetFields:true, withVoice:<voice_consumption_enabled>}`; `data.user.result.timeline.timeline.instructions` |

Several call sites also spread X-main module 891515's shared-variable helper. Its imported implementation was not fully traced; the table is not a claim that additional shared variables can never occur.

Historical IDs for comparison, **not fallback recommendations**:

- Bird shipped `TweetDetail: _NvJCnIjOW__EP5-RF197A` in B-IDs and `97JF30KziU00483E_8elBA` in constants; additional fallback `aFvUsJm2c-oDkJV75blV6g`. Its post-read path uses TweetDetail, not a dedicated single-post query.
- Bird shipped `SearchTimeline: 6AAys3t42mosm_yTI_QENg`; constants/fallbacks include `M1jEez78PEfVfbQLvlWMvQ`, `5h0kNbk3ii97rmfY6CdgAA`, `Tp1sewRU1AsZpBWhqCZicQ`. **X-main module 536462 maps `5h0kNbk3ii97rmfY6CdgAA` to `SharingAudiospacesListeningDataWithFollowersUpdate`, type `mutation`.** This proves the fallback manifest unsafe to assume correct; it does not prove a mismatched request would successfully mutate anything.
- Bird UserByScreenName candidates: `xc8f1g7BYqr6VTzTbvNlGw`, `qW5u-DAuXpMEG0zA1F7UGQ`, `sLVLhk0bGj3MVFEKTdax1w`; variables include historical `withSafetyModeUserFields:true` rather than the current translation flag.
- Bird UserTweets: `Wms1GvIiHXAPBaCr9KblaA`; historical `includePromotedContent:false`, `withVoice:true`, toggle `withArticlePlainText:false`.
- Bird search unusually POSTs `{features,queryId}` with variables in the URL; TweetDetail tries POST after GET 404. Neither behavior should replace the current query adapter's observed GET convention.

### Exact feature-switch sets

The following compact set notation preserves **all names** in current operation metadata. Values are deliberately not asserted to be universally correct: account experiments and logged-out configuration differ.

**Common post set P** — identical for UserTweets, TweetDetail, SearchTimeline:

```text
rweb_video_screen_enabled
rweb_cashtags_enabled
profile_label_improvements_pcf_label_in_post_enabled
responsive_web_profile_redirect_enabled
rweb_tipjar_consumption_enabled
verified_phone_label_enabled
creator_subscriptions_tweet_preview_api_enabled
responsive_web_graphql_timeline_navigation_enabled
premium_content_api_read_enabled
communities_web_enable_tweet_community_results_fetch
c9s_tweet_anatomy_moderator_badge_enabled
responsive_web_grok_analyze_button_fetch_trends_enabled
responsive_web_grok_analyze_post_followups_enabled
rweb_cashtags_composer_attachment_enabled
responsive_web_jetfuel_frame
rweb_sports_post_context_enabled
responsive_web_grok_share_attachment_enabled
responsive_web_grok_annotations_enabled
articles_preview_enabled
responsive_web_edit_tweet_api_enabled
rweb_conversational_replies_downvote_enabled
graphql_is_translatable_rweb_tweet_is_translatable_enabled
view_counts_everywhere_api_enabled
longform_notetweets_consumption_enabled
responsive_web_twitter_article_tweet_consumption_enabled
content_disclosure_indicator_enabled
content_disclosure_ai_generated_indicator_enabled
responsive_web_grok_show_grok_translated_post
responsive_web_grok_analysis_button_from_backend
post_ctas_fetch_enabled
freedom_of_speech_not_reach_fetch_enabled
standardized_nudges_misinfo
tweet_with_visibility_results_prefer_gql_limited_actions_policy_enabled
longform_notetweets_rich_text_read_enabled
longform_notetweets_inline_media_enabled
responsive_web_grok_image_annotation_enabled
responsive_web_grok_imagine_annotation_enabled
responsive_web_grok_community_note_auto_translation_is_enabled
responsive_web_enhance_cards_enabled
```

`TweetResultByRestId` = P minus `rweb_video_screen_enabled` and `responsive_web_enhance_cards_enabled` (ordering differs, membership otherwise identical).

`UserByScreenName` = `hidden_profile_subscriptions_enabled`, `profile_label_improvements_pcf_label_in_post_enabled`, `responsive_web_profile_redirect_enabled`, `rweb_tipjar_consumption_enabled`, `verified_phone_label_enabled`, `subscriptions_verification_info_is_identity_verified_enabled`, `subscriptions_verification_info_verified_since_enabled`, `highlights_tweets_tab_ui_enabled`, `responsive_web_twitter_article_notes_tab_enabled`, `subscriptions_feature_can_gift_premium`, `creator_subscriptions_tweet_preview_api_enabled`, `responsive_web_graphql_timeline_navigation_enabled`.

`Viewer` = `subscriptions_upsells_api_enabled`, `profile_label_improvements_pcf_label_in_post_enabled`, `responsive_web_profile_redirect_enabled`, `rweb_tipjar_consumption_enabled`, `verified_phone_label_enabled`, `creator_subscriptions_tweet_preview_api_enabled`, `responsive_web_graphql_timeline_navigation_enabled`.

Logged-out `/explore` HTML inspection found consistent literal `false` values for these P members: `rweb_video_screen_enabled`, `rweb_tipjar_consumption_enabled`, `verified_phone_label_enabled`, `premium_content_api_read_enabled`, `responsive_web_grok_analyze_button_fetch_trends_enabled`, `responsive_web_grok_analyze_post_followups_enabled`, `rweb_sports_post_context_enabled`, `post_ctas_fetch_enabled`, `longform_notetweets_inline_media_enabled`, `responsive_web_enhance_cards_enabled`; other P members had literal `true`, except `rweb_conversational_replies_downvote_enabled`, which was absent from that simple name/value scan. UserByScreenName's gift-premium switch had both false and true occurrences in separate configuration sections; Viewer upsells had false. These are **observations, not a resolved authenticated feature profile**. Missing flags must not be guessed from Bird's much older true/false dictionaries.

Field-toggle metadata for all four post operations: `withPayments`, `withAuxiliaryUserLabels`, `withArticleRichContentState`, `withArticlePlainText`, `withArticleSummaryText`, `withArticleVoiceOver`, `withGrokAnalyze`, `withDisallowedReplyControls`. UserByScreenName: first two only. Viewer: `isDelegate` plus first two. A declared toggle need not be supplied. Current TweetDetail call site explicitly computes `withArticlePlainText` as a switch **AND false**; Bird's article toggles enable rich content and plain text. Do not promise article completeness based on those historical flags.

### Parsing and pagination

X-main corroborates the top-level roots in the table and discriminates `Tweet`, `TweetWithVisibilityResults`, `TweetUnavailable`, and `UserUnavailable`. B-parsing supplies useful historical nested paths:

- `instructions[].entries[].content.itemContent.tweet_results.result`; module items at `content.items[].item.itemContent.tweet_results.result` (and direct item variants).
- Unwrap `TweetWithVisibilityResults.tweet` before mapping. Never turn unavailable/blocked/protected/deleted/tombstone results into an empty successful post.
- Post ID `rest_id`; author `core.user_results.result.rest_id` and its `core`/legacy profile fields; full note text `note_tweet.note_tweet_results.result.text` before `legacy.full_text`; conversation/reply IDs in `legacy.conversation_id_str` and `legacy.in_reply_to_status_id_str`; quote `quoted_status_result.result`; media from legacy entities/extended entities. Keep metrics optional.
- Bottom cursor: entry `content.cursorType === "Bottom"`, opaque `content.value`; support module/show-more and replacement instructions after fixture verification rather than only Bird's `entries[]` loop. Preserve ordering, deduplicate by stable ID, bound pages, detect repeated cursors, and report retained partial results on later failure.
- GraphQL HTTP 200 can contain errors and data together. Missing expected roots is schema drift/error, not necessarily “zero results.” Same-conversation membership and sorted timestamps do not prove a full thread or reconstruct parent edges.

### Follow-up: user-observed SearchTimeline HTTP 404

After the user reported HTTP 404 from the authenticated search command, public
sources were re-fetched without cookies, browser access or authenticated calls:

- `/explore` still advertises X-main above, whose SHA-256 is unchanged.
  `SearchTimeline` remains query ID `KPSo2_UWdOMpPJwjhfT1Qg`, type `query`.
  `fetchSearchGraphQL` calls the normal GraphQL adapter without `forcePost`.
- The adapter explicitly supplies **`content-type: application/json` on GET**.
  xcli omitted that header; it has now been aligned with the source contract.
  This discrepancy is established, but is **not yet proven to cause the 404**.
- [Vendor bundle](https://abs.twimg.com/responsive-web/client-web/vendor.588f26613fb657c8a.js),
  SHA-256 `121fd93d17747b914c5593917e14cc2c2f12e6849f7ebd4ac8589afdedcd6ad9`,
  supplies module `812055` as an empty object. Thus the shared-variable helper
  inspected earlier does not add missing search variables in this snapshot.
- [Logged-in API filters](https://abs.twimg.com/responsive-web/client-web/bundle.LoggedInApiFilters.31c5a6f3d43634eea.js),
  SHA-256 `91add1aa282df4773696c495afbba785a3671c1ea5e8129a5a607b1d31e6725c`,
  contain no SearchTimeline GET-to-POST rewrite. The inspected session-binding
  operation allowlist does not include SearchTimeline; its presence elsewhere
  is not evidence that search requires those signing operations.

The query ID and GET-only routing are unchanged. No old-ID fallback, POST probe,
transaction-ID fabrication, other-account attempt or public-content fallback
was added. HTTP 404 alone does not establish whether an operation was removed,
a request contract was rejected, or account/client policy obscured the endpoint.
A consented local retry is needed to assess the corrected header. If 404 remains,
compare only the successful browser request's method and operation path (not
cookies, headers, query variables, a HAR, or a copied cURL command) before changing
transport or IDs.

## 3. macOS Chrome credential access: supported boundary

First-party source snapshot: Chromium **`233e625e16284f1f1e11150b88ea16e43c325c37`**, inspected 2026-09-10. This is mainline source, **not a verified installed/released Chrome build**:

- [C-key-provider](https://github.com/chromium/chromium/blob/233e625e16284f1f1e11150b88ea16e43c325c37/components/os_crypt/async/browser/keychain_key_provider.mm)
- [C-password](https://github.com/chromium/chromium/blob/233e625e16284f1f1e11150b88ea16e43c325c37/components/os_crypt/common/keychain_password_mac.mm), [C-Keychain API](https://github.com/chromium/chromium/blob/233e625e16284f1f1e11150b88ea16e43c325c37/crypto/apple/keychain_v2.mm)
- [C-encryptor](https://github.com/chromium/chromium/blob/233e625e16284f1f1e11150b88ea16e43c325c37/components/os_crypt/async/common/encryptor.cc)
- [C-cookie-store](https://github.com/chromium/chromium/blob/233e625e16284f1f1e11150b88ea16e43c325c37/net/extras/sqlite/sqlite_persistent_cookie_store.cc)
- [C-profile directories](https://github.com/chromium/chromium/blob/233e625e16284f1f1e11150b88ea16e43c325c37/docs/user_data_dir.md)
- Historical corroboration: [macOS sync crypto at tag `143.0.7497.1`](https://github.com/chromium/chromium/blob/143.0.7497.1/components/os_crypt/sync/os_crypt_mac.mm). That sync path is absent on inspected main; use the async sources above for current claims.

Recommended initial support contract (design, not tested extraction):

1. **Explicit profile opt-in.** Chrome Stable's documented macOS user-data root is `~/Library/Application Support/Google/Chrome`; profiles are separate children such as `Default`/`Profile 1`. Use explicit selection and safe metadata-only discovery. Verify the selected build's cookie location/schema; do not assume a Windows `Network/Cookies` layout applies to every macOS version. No real profile was inspected here.
2. **OS-supported read, not protection bypass.** Use Security.framework `SecItemCopyMatching` for the existing generic password, service **`Chrome Safe Storage`**, account **`Chrome`**, as corroborated by C-password/C-Keychain API. Chromium uses `Chromium Safe Storage`/`Chromium`, a different brand. Respect system authorization prompts and denial/locked-Keychain errors. Never create/reset the browser's Keychain item: Chromium itself may create one on “not found,” but this read-only tool must not copy that behavior. Native in-process access avoids a shell command printing key material.
3. **Restrict DB access.** Read only the selected cookie database with a consistent read-only SQLite transaction/backup strategy that includes WAL state; copying just a live main DB can miss current cookies. Prefer an in-memory snapshot and no complete cookie dump. If locked/inconsistent, ask the user to close Chrome; do not kill it or change database permissions. Restrict any unavoidable temporary files, remove them reliably, and never use `immutable=1` on a changing live DB as a consistency shortcut.
4. **Select minimally.** Only `auth_token`/`ct0` applicable to the exact X request host/path, from the same selected profile and coherent domain/session. Respect expiry, secure flag, host-only versus domain cookies, path and partition context; reject ambiguity. Do not match arbitrary suffixes such as `evilx.com`, merge X and legacy Twitter jars blindly, or treat HttpOnly as “not needed.” Reject unsupported partitioned-cookie cases rather than silently selecting them. Do not infer independent sessions from X's in-profile account switcher.
5. **Verified `v10` decoding.** C-key-provider derives **16 bytes** using PBKDF2-HMAC-SHA1 over the Keychain password bytes, salt `saltysalt`, **1003 iterations**. Strip the three-byte ASCII `v10` provider tag, then AES-128-CBC with IV **16 ASCII spaces** and strict PKCS#7 padding validation. The stored password's textual bytes are the KDF input; do not base64-decode it merely because Chrome generated base64-looking text.
6. **Host digest is mandatory for schema 24.** Inspect `meta.version` and compatibility/schema before decoding. In schema 24, encrypted plaintext is **`SHA256(host_key exact stored bytes) || cookie_value`**. Compare the first **32 raw bytes**, including the leading dot in the hashed domain when stored, then strip only on successful verification. Do not hex-encode the digest, hash the cookie name, blindly drop 32 bytes, or decode the binary digest as UTF-8. C-cookie-store currently declares version/compatible-version 24 and rejects digest mismatch. Earlier schemas need explicitly versioned fixtures; unknown future schemas fail closed. If both plaintext `value` and `encrypted_value` are nonempty, current Chromium rejects the row. Plaintext storage, if intentionally supported, must not be subjected to encrypted-value digest stripping.
7. **Protected/unknown formats are blockers.** The inspected Windows [app-bound provider header](https://github.com/chromium/chromium/blob/233e625e16284f1f1e11150b88ea16e43c325c37/chrome/browser/os_crypt/app_bound_encryption_provider_win.h) defines data tag `v20` and wrapped-key marker `APPB`; [implementation](https://github.com/chromium/chromium/blob/233e625e16284f1f1e11150b88ea16e43c325c37/chrome/browser/os_crypt/app_bound_encryption_provider_win.cc) uses AES-256-GCM and OS app-bound services. This is **not** macOS `v10`, and prefix alone is not a cross-platform decoding specification. Reject `v20`, other unknown tags, unavailable/non-exportable keys, and unauthorized access for initial macOS support. No process injection, elevated bypass, browser remote-debugging workaround, security-policy downgrade, mock/empty-password fallback, or key-generation fallback. C-encryptor has a compatibility empty-key fallback; deliberately exclude it from this support contract.
8. Keep both session cookies and the much broader Safe Storage key out of logs, model/tool transcripts, environment variables, process arguments, normal output and third-party requests. Minimize lifetime and zeroize practical buffers. Persist only profile reference/expected stable identity; if secrets must persist, use a dedicated OS credential-store item. Decryption success proves storage compatibility, **not authenticated identity**: verify Viewer before registering/reusing the connection and reject silent ID changes.

## 4. Implementation recommendations and remaining blockers

1. Proceed with unauthenticated FxTwitter reads separately. Never send X session headers to that backend or silently substitute it for an explicitly account-selected request.
2. Keep a small reviewed **allowlist of `(operationName, queryId, operationType=query, method, feature names, toggles, source URL/hash)`**. Discover/parse adjacent object definitions without evaluating remote JS. Reject duplicates/conflicts and any mutation; IDs alone or regexes spanning unrelated modules are unsafe. Never blindly recycle Bird's fallback IDs. A refresh may occur once for convincing manifest mismatch, not on every 401/403/429.
3. Initial GraphQL transport should be GET-only for these observed queries, strict HTTPS X-host allowlist, no cross-origin credential redirects, bounded request/response sizes, account-isolated cache/rate-limit state, and redacted typed errors. Read-only is an application allowlist, not a limitation on the stolen/reused session's power.
4. **Not yet established:** live Viewer identity semantics for a consenting account; exact required authenticated headers/transaction behavior; released-Chrome profile database path/build support; feature values for authenticated rollouts; complete response schemas and pagination branches. No claim that the IDs above currently execute successfully is justified without those tests.
5. Before shipping auth: opt-in local tests with user-controlled accounts, sanitized fixtures for identity mismatch, wrapped/unavailable posts, note text, mixed GraphQL errors, cursor replacement/repetition, expiry, 401/403/429, operation churn and partial pages. Storage tests should be synthetic: schema 23/24, exact-domain digest mismatch, bad padding, malformed/unknown prefixes, conflicting rows, partition ambiguity, Keychain denial and SQLite WAL. Do not send raw fixtures containing cookies or private account data to an agent.
6. Stop on identity uncertainty, unsupported protection/schema, consent denial, rate limits or missing authoritative definitions; explain the blocker rather than trying another profile/account, bypassing protections, or claiming completeness. Unofficial endpoints remain subject to X policy/enforcement and can break without notice.
