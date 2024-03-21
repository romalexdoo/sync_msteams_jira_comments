
# sync_msteams_jira_comments

This small project is aimed to sync users who work in MS Teams and support team working in Jira


## About
Our support team works in Atlassian Jira (Cloud) and supports business users who work in Microsoft Teams. We tried to find a solution how to integrate support process between MS Teams and Jira and faced the truth that there no easy out of the box solution for us. So, we've built our own tool.

## How it works

 - Business users initiate requests by creation of a new post in a specified MS Teams channel => our tool creates issue in Jira automatically and reply to this post with a link to Jira issue
 - When someone adds replies to MS Teams post, these replies are automatically transferred to Jira comments
 - When support team moves issue to a new status, status update reply is created in MS Teams providing business users information about the progress
 - Sometimes users try to continue conversation or try to init new request for the closed topic. MS Teams can't deny that, so we are) But our tool in such cases notifies user that his request can be missed and that they should open new request instead.

## Setup environment
### Configure Microsoft API
Here we'll configure our tool to have access to MS Graph API. Actually, you can read it [here](https://learn.microsoft.com/en-us/graph/auth/?context=graph/api/1.0&view=graph-rest-1.0), but it's not an easy task to get through MS documentation, so here is a shorter version:

 1. Login to your tenant admin center ([Entra](https://entra.microsoft.com)) and go to **Applications -> App registrations**
 2. Click **New registration** and go through the process:
    - Name = any
    - Redirect URI = Public client / native
 
 3. In this new application go to **Certificates and secrets** and add new **Client secret**. **Important!** Don't forget to save **Secret value** as you'll need it later
 4. Go to **API permissions**. Click **Add a permission**, choose **APIs my organisation uses**, find **Microsoft Graph** and click on it
 5. Select **Application permissions** and add required permissions:
    - `ChannelMessage.Read.Group`, `ChannelMessage.Read.All` to read messages in the channel
    - `User.Read.All` to read user properties (we need to know user's email)
    - `Mail.Send` to send approve link to service desk mailbox
    - `ChannelMessage.Read.Group`, `ChannelMessage.Read.All` to manipulate the subscription (webhook notification about channel updates)
 6. Click again **Add a permission**, choose **APIs my organisation uses**, find **Microsoft Graph**, click on it and this time select **Delegated permissions**. Here add `offline_access` and `ChannelMessage.Send`
 7. You'll see the list of added permissions. Now click `Grant admin consent`
 8. Go to the **Authentication** on  the left and add the following url: `https://<your domain>/ms_oauth`
 9.  Go to **Overview** and copy:
	 - Application (client) ID
	 - Directory (tenant) ID
 10. Create a service desk user mailbox. This user should have mailbox to send/receive emails and MS Teams subscription to reply to messages in MS Teams. Don't forget to give access for this user to the MS Teams support channel

### Configure Jira API
 1. To add a webhook
	 1. Go to **Settings** –> **System and find** [Webhooks](https://plnew.atlassian.net/plugins/servlet/webhooks) on the panel
	 2. Click Create new **webhook**
		 - Define name (any)
		 - Edit url: `https://<your domain>/jira`
		 - Generate secret and write it down
		 - Edit jql query to narrow the search (for ex., `project = "IT support"`)
		 - Below select only **Issue** –> **Updated** checkbox
 2. Add service desk user to your support project with writes to read, create and edit issues, add and edit comments
 3. Go to this user's [Manage account page](https://id.atlassian.com/manage-profile/profile-and-visibility), goto **Security** tab, click **Create and manage API tokens** and create new token (don't forget to copy the **Token value**)
 4. Create custom field in Jira to store link to MS Teams. I suggest to make this field's type: URL to be able to easily open teams message if needed

### Configure your server
You would need a web server with DNS name available from internet.

 1. Install Rust: https://www.rust-lang.org/tools/install
 2. When build is successful, create folder `/opt/sync_msteams_jira_comments`

### Configure service

 1. git clone source code to your server
 2. Try to build-and-run using `./backend.sh` script (for Windows environment you'll have to translate sh-script to bat-file or to use WSL)
 3. Sometimes it's needed to install extra libraries, for ex., last time we had to install libssl-dev on our Ubuntu server
 4. When build is successful, create folder `/opt/sync_msteams_jira_comments`
 5. Copy `.env` file from `deploy` source folder
 6. Edit .env file with:
	 - `API_ADDR` = `0.0.0.0:443` (for direct requests) or `127.0.0.1:<port>` (for proxy requests)
	 - `SHUTDOWN_TIMEOUT` = timeout for graceful stop the app if not yet stopped
	 - `MICROSOFT_TENANT_ID`, `MICROSOFT_CLIENT_ID`, `MICROSOFT_CLIENT_SECRET` you've got them when setting up Microsoft API
	 - `MICROSOFT_SUBSCRIPTION_NOTIFICATION_URL` =  `https://<your domain>/ms_oauth`
	 - `MICROSOFT_SUBSCRIPTION_LIFECYCLE_NOTIFICATION_URL` =  `https://<your domain>/teams`
	 - `MICROSOFT_OAUTH_URL` =  `https://<your domain>/teams_lifecycle`
	 - `TEAMS_GROUP_ID` and `TEAMS_CHANNEL_ID` you can get by copying the link to this group
	 - `TEAMS_USER` and `JIRA_USER` is the email of your service desk user account
	 - `JIRA_SECRET` – your generated subscription secret
	 - `JIRA_TOKEN` – you service desk user's API token
	 - `JIRA_BASE_URL` – your Jira's base url: `https://<your jira prefix>.atlassian.net`
	 - `JIRA_PROJECT_KEY` – the key of the support project in Jira
	 - `JIRA_MSTEAMS_LINK_FIELD_NAME` and `JIRA_MSTEAMS_LINK_FIELD_JQL_NAME` are internal name of the added field (e.g. `customfield_????`) and the name of this field that you can use in JQL query (for ex., `MS Teams link[URL Field]`)
 7. OK, now configure the tool to run as a service. There are 2 pre-configured files in `deploy` folder: one contain `systemd` config, second one is a bash script to be run when service starts (copy it to `/opt/sync_msteams_jira_comments` folder)
 8. Now you can just run `./build.sh` script. It takes the latest version from Github, build and restart the service
 9. Enjoy!

## Our plans

 - Add language selection (for now all responses are in Russian language)
 - Add configuration for "Final" Jira statuses
 - Add option to reopen issues in final statuses on new comments
 - Add option to transfer comments from Jira to MS Teams. Why option? Because sometimes technical team may need to leave comments without notifying users, so for now there is only one-way comments sync
