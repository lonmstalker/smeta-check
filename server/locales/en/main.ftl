# API response texts. Key = machine-readable error code for the frontend.

error-internal = Internal error, please try again later
error-unauthorized = Sign-in required
error-forbidden = Not enough permissions
error-not-found = Not found
error-validation-email = Invalid email address
error-password-short = Password must be at least { $min } { $min ->
        [one] character
       *[other] characters
    }
error-email-taken = This email is already taken
error-invalid-credentials = Invalid email or password
error-invalid-totp = Invalid verification code
error-totp-already-enabled = Two-factor authentication is already enabled
error-totp-not-enabled = Two-factor authentication is not enabled
error-invalid-token = The link is invalid or expired
error-title-empty = Title must not be empty
error-too-many-requests = Too many attempts, please wait a minute
error-wrong-password = Current password is incorrect
error-no-password = This account has no password yet — set one via password recovery
error-email-same = This is already your address
error-name-long = Name must be at most { $max } characters
error-unknown-locale = Unknown language
error-oauth-not-configured = Sign-in via { $provider } is not configured
error-oauth-failed = Failed to sign in via { $provider }
error-estimate-no-file = No file received — attach the estimate and try again
error-estimate-format = For now we only accept Excel: xlsx and xls files
error-estimate-empty = The file is empty — check that the estimate was saved
error-estimate-too-large = The file is larger than { $max } MB — send the estimate without extra images
error-estimate-limit = For now you can keep at most { $max } { $max ->
        [one] estimate
       *[other] estimates
    }

email-verify-subject = Confirm your email address
email-verify-body = To confirm your address, open the link: { $link }
    The link is valid for { $hours } { $hours ->
        [one] hour
       *[other] hours
    } and works only once.

email-reset-subject = Password recovery
email-reset-body = To set a new password, open the link: { $link }
    The link is valid for { $minutes } { $minutes ->
        [one] minute
       *[other] minutes
    } and works only once.

email-change-subject = Confirm your new email address
email-change-body = Open this link to activate the new address: { $link }
    The link works for { $minutes } { $minutes ->
        [one] minute
       *[other] minutes
    } and only once. Until you open it, sign-in stays on the old address.

email-change-notice-subject = Email change requested
email-change-notice-body = Someone asked to change this account's address to { $email }.
    If that was not you, change your password: the switch only happens after
    confirmation from the new address.
