# Nginx Compose implementation plan

> **For agentic workers:** Use executing-plans with checkable evidence and independentreview before finaldelivery.

**Goal:** One Compose file starts nginxfrontend+internalbackend while preservingregistrationandaccess.
**Architecture:** nginxsolepublicentry, builtstaticassets, unchangedbackendprotocolpaths andauthority; optionalGPUprofileforSentiment.
**Spec:** ../specs/2026-11-28-nginx-compose-design.md

## Review focus
Hostwithport/Origin/sessioncookies;MCPstreaming+401metadata;specialpublicroutes;noauthquerylogging;nonloopbackbootstrapguard;existingnamedvolumes;defaultSentimentnoGPUrequirement.

### Task1: Images and Compose
- [ ] Add nginxconfig/frontendimage; preserveexistingstandaloneimage andnonrootbackend.
- [ ] Onecompose withinternalbackend, originalvolume/port names, matchingcanonicalURL; Sentimentconsolidatesoptionalinferenceprofile.
- [ ] DockercomposeconfigJSON andnginx-t provevalidrouting,onepublicappport,nosecrets.

### Task2: Integration verification
- [ ] Buildactualimages andstartisolatedtemporaryComposeproject,neverproductiondata.
- [ ] Verifyfrontendstatic/deeplinks and registration/adminreview/cookies throughnginx.
- [ ] CheckcrossOriginblocked,OAuthmetadata/challenge, MCPstreamsupportconfig, specialroutes,redactedlogs.
- [ ] Recordtruthfulresults/remainingGPUorSSOlimits in docs/nginx-deployment.md.

### Task3: Delivery
- [ ] UpdateREADMEonecommanddiagram/config; commitandindependentlyreviewproxychange.
- [ ] PRchecks/localequivalents;merge/deleteauthorizedbranch;verifycleanmainandpreservedvisibility.
- [ ] Removeonlyownedtestcontainers/volumes/tempcredentials andrecordfinaldelivery.
